use crate::i18n::{message, LocalizedMessage};
use crate::steam_util::SteamUtil;
use crate::wine_cask::download_progress::DownloadProgress;
use crate::wine_cask::flavors::{
    CatalogRelease, CompatibilityToolFlavor, Flavor, InstalledCompatibilityTool,
    SteamClientCompatToolInfo,
};
use crate::PeerMap;
use log::{debug, error, info, warn};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::{Mutex, Notify};
use tokio_tungstenite::tungstenite::Message;

const DOWNLOAD_PROGRESS_BROADCAST_INTERVAL: Duration = Duration::from_millis(200);
const APP_COMPAT_TOOL_MAPPINGS_REFRESH_INTERVAL: Duration = Duration::from_millis(500);

pub struct WineCask {
    pub steam_util: SteamUtil,
    pub app_state: Arc<Mutex<AppState>>,
    pub operation_broadcast_cache: Arc<Mutex<Option<(OperationStateSnapshot, Instant)>>>,
    pub queue_notify: Arc<Notify>,
}

#[derive(Clone)]
pub struct QueuedCommand {
    pub command: Command,
    pub operation: OperationInfo,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct AppState {
    pub catalog_flavors: Vec<Flavor>,
    pub installed_tools: Vec<InstalledCompatibilityTool>,
    pub app_compat_tool_mappings: HashMap<u64, String>,
    pub app_compat_tool_mappings_stale: bool,
    pub current_operation: Option<OperationInfo>,
    pub queued_operations: Vec<OperationInfo>,
    pub updater_state: UpdaterState,
    pub updater_last_check: Option<u64>,
    pub steam_visible_tools: Vec<SteamClientCompatToolInfo>,
    #[serde(skip)]
    pub operation_queue: VecDeque<QueuedCommand>,
    #[serde(skip)]
    pub app_compat_tool_mappings_last_refresh_attempt: Option<Instant>,
    #[serde(skip)]
    pub app_compat_tool_mappings_refresh_in_progress: bool,
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub enum UpdaterState {
    Idle,
    Checking,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub enum MessageType {
    GetState,
    ReportSteamVisibleTools,
    Command,
    UpdateState,
    UpdateOperations,
    Notification,
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct OperationStateSnapshot {
    pub current_operation: Option<OperationInfo>,
    pub queued_operations: Vec<OperationInfo>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct MessageEnvelope {
    pub r#type: MessageType,
    pub command: Option<Command>,
    pub notification: Option<LocalizedMessage>,
    pub steam_visible_tools: Option<Vec<SteamClientCompatToolInfo>>,
    pub app_state: Option<AppState>,
    pub operation_state: Option<OperationStateSnapshot>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(tag = "type")]
pub enum Command {
    RefreshCatalog,
    InstallCatalogRelease {
        release_id: String,
    },
    UninstallInstalledTool {
        installed_tool_id: String,
    },
    CancelOperation {
        operation_id: String,
    },
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub enum OperationKind {
    Install,
    Uninstall,
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub enum OperationState {
    Pending,
    Running,
    Downloading,
    Extracting,
    Cancelling,
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct OperationInfo {
    pub id: String,
    pub label: LocalizedMessage,
    pub kind: OperationKind,
    pub state: OperationState,
    pub progress: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub download: Option<DownloadProgress>,
    pub release_id: Option<String>,
    pub installed_tool_id: Option<String>,
}

impl WineCask {
    pub(crate) async fn begin_next_operation(&self, peer_map: &PeerMap) -> Option<QueuedCommand> {
        let mut app_state = self.app_state.lock().await;
        let next_operation = app_state.operation_queue.pop_front();
        app_state.queued_operations = app_state
            .operation_queue
            .iter()
            .map(|queued| queued.operation.clone())
            .collect();
        app_state.current_operation = next_operation
            .as_ref()
            .map(|queued| queued.operation.clone());
        drop(app_state);
        self.broadcast_operation_state(peer_map).await;
        next_operation
    }

    pub async fn complete_current_operation(&self, peer_map: &PeerMap) {
        self.app_state.lock().await.current_operation = None;
        self.broadcast_operation_state(peer_map).await;
    }

    pub async fn update_current_operation(
        &self,
        state: OperationState,
        progress: u8,
        peer_map: &PeerMap,
    ) {
        let mut app_state = self.app_state.lock().await;
        if let Some(operation) = &mut app_state.current_operation {
            if operation.state != OperationState::Cancelling || state == OperationState::Cancelling
            {
                operation.state = state;
            }
            operation.progress = progress;
            if operation.state != OperationState::Downloading {
                operation.download = None;
            }
        }
        drop(app_state);
        self.broadcast_operation_state(peer_map).await;
    }

    pub async fn update_current_download(&self, download: DownloadProgress, peer_map: &PeerMap) {
        let mut app_state = self.app_state.lock().await;
        if let Some(operation) = &mut app_state.current_operation {
            if operation.state != OperationState::Cancelling {
                operation.state = OperationState::Downloading;
                operation.progress = download.percentage();
                operation.download = Some(download);
            }
        }
        drop(app_state);
        self.broadcast_operation_state(peer_map).await;
    }

    pub async fn current_operation_is_cancelling(&self) -> bool {
        self.app_state
            .lock()
            .await
            .current_operation
            .as_ref()
            .map(|operation| operation.state == OperationState::Cancelling)
            .unwrap_or(false)
    }

    pub async fn queue_install_catalog_release(
        &self,
        release_id: String,
        peer_map: &PeerMap,
    ) {
        let Some(catalog_release) = self.get_catalog_release(&release_id).await else {
            self.broadcast_notification(peer_map, message("notification-unknownRelease", &[]))
                .await;
            return;
        };

        let label = message(
            "operation-install",
            &[("tool", &catalog_release.release.tag_name)],
        );

        let mut app_state = self.app_state.lock().await;
        if app_state.installed_tools.iter().any(|tool| {
            tool.catalog_release_id.as_deref() == Some(catalog_release.id.as_str())
        }) {
            drop(app_state);
            self.broadcast_notification(peer_map, message("notification-alreadyInstalled", &[]))
                .await;
            return;
        }

        let operation_conflicts = |operation: &OperationInfo| {
            operation.kind == OperationKind::Install
                && operation.release_id.as_deref() == Some(release_id.as_str())
        };
        if app_state
            .current_operation
            .as_ref()
            .map(operation_conflicts)
            .unwrap_or(false)
            || app_state
                .operation_queue
                .iter()
                .any(|queued| operation_conflicts(&queued.operation))
        {
            drop(app_state);
            self.broadcast_notification(peer_map, message("notification-alreadyQueued", &[]))
                .await;
            return;
        }

        let queued_command = QueuedCommand {
            command: Command::InstallCatalogRelease { release_id },
            operation: OperationInfo {
                id: operation_id(),
                label,
                kind: OperationKind::Install,
                state: OperationState::Pending,
                progress: 0,
                download: None,
                release_id: Some(catalog_release.id),
                installed_tool_id: None,
            },
        };

        app_state.operation_queue.push_back(queued_command);
        app_state.queued_operations = app_state
            .operation_queue
            .iter()
            .map(|queued| queued.operation.clone())
            .collect();
        drop(app_state);
        self.queue_notify.notify_one();
        self.broadcast_operation_state(peer_map).await;
    }

    pub async fn queue_uninstall_installed_tool(
        &self,
        installed_tool_id: String,
        peer_map: &PeerMap,
    ) {
        let Some(installed_tool) = self.get_installed_tool(&installed_tool_id).await else {
            self.broadcast_notification(peer_map, message("notification-unknownInstalledTool", &[]))
                .await;
            return;
        };

        let label = message("operation-remove", &[("tool", &installed_tool.display_name)]);

        let mut app_state = self.app_state.lock().await;
        if app_state
            .current_operation
            .as_ref()
            .map(|operation| operation_targets_installed_tool(operation, &installed_tool_id))
            .unwrap_or(false)
            || app_state.operation_queue.iter().any(|queued| {
                operation_targets_installed_tool(&queued.operation, &installed_tool_id)
            })
        {
            drop(app_state);
            self.broadcast_notification(
                peer_map,
                message("notification-toolBusy", &[]),
            )
            .await;
            return;
        }
        let queued_command = QueuedCommand {
            command: Command::UninstallInstalledTool { installed_tool_id },
            operation: OperationInfo {
                id: operation_id(),
                label,
                kind: OperationKind::Uninstall,
                state: OperationState::Pending,
                progress: 0,
                download: None,
                release_id: installed_tool.catalog_release_id.clone(),
                installed_tool_id: Some(installed_tool.id.clone()),
            },
        };

        app_state.operation_queue.push_back(queued_command);
        app_state.queued_operations = app_state
            .operation_queue
            .iter()
            .map(|queued| queued.operation.clone())
            .collect();
        drop(app_state);
        self.queue_notify.notify_one();
        self.broadcast_operation_state(peer_map).await;
    }

    pub async fn cancel_operation(&self, operation_id: String, peer_map: &PeerMap) {
        let mut app_state = self.app_state.lock().await;
        if let Some(position) = app_state
            .operation_queue
            .iter()
            .position(|queued| queued.operation.id == operation_id)
        {
            app_state.operation_queue.remove(position);
            app_state.queued_operations = app_state
                .operation_queue
                .iter()
                .map(|queued| queued.operation.clone())
                .collect();
            drop(app_state);
            self.broadcast_operation_state(peer_map).await;
            self.broadcast_notification(peer_map, message("notification-cancelledQueued", &[]))
                .await;
            return;
        }

        if let Some(operation) = &mut app_state.current_operation {
            if operation.id == operation_id {
                if operation.kind == OperationKind::Install {
                    operation.state = OperationState::Cancelling;
                    operation.download = None;
                    drop(app_state);
                    self.broadcast_operation_state(peer_map).await;
                    self.broadcast_notification(peer_map, message("notification-cancellingInstall", &[]))
                        .await;
                    return;
                }

                drop(app_state);
                self.broadcast_notification(
                    peer_map,
                    message("notification-onlyInstallsCancellable", &[]),
                )
                .await;
                return;
            }
        }

        drop(app_state);
        self.broadcast_notification(peer_map, message("notification-operationNotFound", &[]))
            .await;
    }

    pub async fn broadcast_app_state(&self, peer_map: &PeerMap) {
        let app_state = self.app_state.lock().await;
        let response_new = MessageEnvelope {
            r#type: MessageType::UpdateState,
            command: None,
            notification: None,
            steam_visible_tools: None,
            app_state: Some(app_state.clone()),
            operation_state: None,
        };
        drop(app_state);
        self.broadcast_message(peer_map, &response_new).await;
    }

    pub async fn broadcast_operation_state(&self, peer_map: &PeerMap) {
        let snapshot = {
            let app_state = self.app_state.lock().await;
            OperationStateSnapshot {
                current_operation: app_state.current_operation.clone(),
                queued_operations: app_state.queued_operations.clone(),
            }
        };

        let now = Instant::now();
        let mut operation_broadcast_cache = self.operation_broadcast_cache.lock().await;
        if should_skip_operation_broadcast(operation_broadcast_cache.as_ref(), &snapshot, now) {
            return;
        }

        *operation_broadcast_cache = Some((snapshot.clone(), now));
        drop(operation_broadcast_cache);

        let response_new = MessageEnvelope {
            r#type: MessageType::UpdateOperations,
            command: None,
            notification: None,
            steam_visible_tools: None,
            app_state: None,
            operation_state: Some(snapshot),
        };
        self.broadcast_message(peer_map, &response_new).await;
    }

    pub async fn broadcast_notification(&self, peer_map: &PeerMap, message: LocalizedMessage) {
        let response_new = MessageEnvelope {
            r#type: MessageType::Notification,
            command: None,
            notification: Some(message),
            steam_visible_tools: None,
            app_state: None,
            operation_state: None,
        };
        self.broadcast_message(peer_map, &response_new).await;
    }

    async fn broadcast_message(&self, peer_map: &PeerMap, response: &MessageEnvelope) {
        let update = match serde_json::to_string(response) {
            Ok(update) => update,
            Err(err) => {
                error!("Failed to serialize websocket response: {}", err);
                return;
            }
        };

        let message = Message::text(&update);
        for recipient in peer_map.lock().await.values() {
            match recipient.unbounded_send(message.clone()) {
                Ok(_) => {
                    info!("Type: {:?}", response.r#type);
                    debug!("Websocket message sent: {}", &update);
                }
                Err(err) => {
                    error!("Failed to send websocket message: {}", err);
                }
            }
        }
    }

    fn get_used_by_games(&self, display_name: &str, internal_name: &str) -> Vec<String> {
        let compat_tools_mapping = self
            .steam_util
            .get_compatibility_tools_mappings()
            .unwrap_or_else(|err| {
                warn!("Failed to get compatibility tools mappings: {}", err);
                HashMap::new()
            });
        let installed_games = self
            .steam_util
            .list_installed_games()
            .unwrap_or_else(|err| {
                warn!("Failed to get list of installed games: {}", err);
                Vec::new()
            });

        installed_games
            .iter()
            .filter(|game| {
                compat_tools_mapping
                    .get(&game.app_id)
                    .map(|name| name == display_name || name == internal_name)
                    .unwrap_or(false)
            })
            .map(|game| game.name.clone())
            .collect()
    }

    pub fn list_compatibility_tools(&self) -> Option<Vec<InstalledCompatibilityTool>> {
        let compat_tools = self.steam_util.list_compatibility_tools().ok()?;

        let mut installed_tools = Vec::new();

        for compat_tool in &compat_tools {
            let used_by_games =
                self.get_used_by_games(&compat_tool.display_name, &compat_tool.internal_name);
            installed_tools.push(InstalledCompatibilityTool {
                id: format!("installed:{}", compat_tool.directory_name),
                path: compat_tool.path.to_string_lossy().to_string(),
                directory_name: compat_tool.directory_name.clone(),
                display_name: compat_tool.display_name.clone(),
                internal_name: compat_tool.internal_name.clone(),
                used_by_games,
                flavor: CompatibilityToolFlavor::Unknown,
                github_release: None,
                catalog_release_id: None,
                requires_restart: false,
            });
        }

        Some(installed_tools)
    }

    pub async fn process_frontend_compat_tools_update(
        &self,
        peer_map: &PeerMap,
        steam_visible_tools: Vec<SteamClientCompatToolInfo>,
    ) {
        self.app_state.lock().await.steam_visible_tools = steam_visible_tools;
        self.sync_backend_state().await;
        self.broadcast_app_state(peer_map).await;
    }

    pub async fn refresh_app_compat_tool_mappings(&self) {
        let now = Instant::now();
        let should_refresh = {
            let mut app_state = self.app_state.lock().await;
            let was_recently_attempted = app_state
                .app_compat_tool_mappings_last_refresh_attempt
                .is_some_and(|last_attempt| {
                    now.saturating_duration_since(last_attempt)
                        < APP_COMPAT_TOOL_MAPPINGS_REFRESH_INTERVAL
                });

            if app_state.app_compat_tool_mappings_refresh_in_progress || was_recently_attempted {
                false
            } else {
                app_state.app_compat_tool_mappings_last_refresh_attempt = Some(now);
                app_state.app_compat_tool_mappings_refresh_in_progress = true;
                true
            }
        };

        if !should_refresh {
            return;
        }

        // config.vdf may live on slow or unhealthy storage. Keep its synchronous file read and
        // VDF parsing off the async runtime, while the in-progress flag prevents concurrent reads.
        let steam_util = self.steam_util.clone();
        let refresh_result = tokio::task::spawn_blocking(move || {
            steam_util.get_forced_compatibility_tools_mappings()
        })
        .await;

        let mut app_state = self.app_state.lock().await;
        app_state.app_compat_tool_mappings_refresh_in_progress = false;
        match refresh_result {
            Ok(Ok(mappings)) => {
                app_state.app_compat_tool_mappings = mappings;
                app_state.app_compat_tool_mappings_stale = false;
            }
            Ok(Err(err)) => {
                // A transient read or parse failure must not erase assignments already shown to
                // the frontend. A later refresh can replace this last-known-good snapshot.
                app_state.app_compat_tool_mappings_stale = true;
                warn!(
                    "Failed to refresh forced compatibility tool mappings; retaining last known state: {}",
                    err
                );
            }
            Err(err) => {
                app_state.app_compat_tool_mappings_stale = true;
                error!(
                    "Compatibility tool mapping refresh task failed; retaining last known state: {}",
                    err
                );
            }
        }
    }

    pub async fn sync_backend_state(&self) {
        self.refresh_app_compat_tool_mappings().await;

        let (catalog_flavors, steam_visible_tools) = {
            let app_state = self.app_state.lock().await;
            (
                app_state.catalog_flavors.clone(),
                app_state.steam_visible_tools.clone(),
            )
        };

        let visible_tool_names: HashSet<String> = steam_visible_tools
            .iter()
            .map(|tool| tool.str_tool_name.clone())
            .collect();

        let mut installed_tools = self.list_compatibility_tools().unwrap_or_default();
        for installed_tool in &mut installed_tools {
            installed_tool.requires_restart =
                !visible_tool_names.contains(&installed_tool.internal_name);
            if let Some(catalog_release) =
                find_catalog_release_for_tool(&catalog_flavors, installed_tool)
            {
                apply_catalog_release(installed_tool, &catalog_release);
            }
        }

        let mut app_state = self.app_state.lock().await;
        app_state.installed_tools = installed_tools;
    }

    pub async fn check_for_flavor_updates(&self, peer_map: &PeerMap, renew_cache: bool) {
        self.app_state.lock().await.updater_state = UpdaterState::Checking;
        self.broadcast_app_state(peer_map).await;
        self.app_state.lock().await.catalog_flavors = self.get_flavors(renew_cache).await;
        self.sync_backend_state().await;
        self.app_state.lock().await.updater_state = UpdaterState::Idle;
        self.broadcast_app_state(peer_map).await;
    }

    pub async fn get_catalog_release(&self, release_id: &str) -> Option<CatalogRelease> {
        self.app_state
            .lock()
            .await
            .catalog_flavors
            .iter()
            .flat_map(|flavor| flavor.releases.iter())
            .find(|release| release.id == release_id)
            .cloned()
    }

    pub async fn get_installed_tool(
        &self,
        installed_tool_id: &str,
    ) -> Option<InstalledCompatibilityTool> {
        self.app_state
            .lock()
            .await
            .installed_tools
            .iter()
            .find(|tool| tool.id == installed_tool_id)
            .cloned()
    }

}

fn operation_id() -> String {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Failed to calculate current timestamp")
        .as_nanos();
    format!("operation-{}", timestamp)
}

fn find_catalog_release_for_tool(
    catalog_flavors: &[Flavor],
    installed_tool: &InstalledCompatibilityTool,
) -> Option<CatalogRelease> {
    catalog_flavors.iter().find_map(|flavor| {
        flavor.releases.iter().find_map(|catalog_release| {
            if flavor.flavor == CompatibilityToolFlavor::ProtonGE {
                if proton_ge_tool_name_matches_release(
                    &installed_tool.internal_name,
                    &catalog_release.release.tag_name,
                ) || proton_ge_tool_name_matches_release(
                    &installed_tool.display_name,
                    &catalog_release.release.tag_name,
                ) {
                    return Some(catalog_release.clone());
                }
            } else if flavor.flavor == CompatibilityToolFlavor::ProtonCachyOS {
                if [
                    &installed_tool.internal_name,
                    &installed_tool.display_name,
                    &installed_tool.directory_name,
                ]
                .iter()
                .any(|name| {
                    proton_cachyos_tool_name_matches_release(
                        name,
                        &catalog_release.release.tag_name,
                    )
                }) {
                    return Some(catalog_release.clone());
                }
            } else if installed_tool.display_name
                == format!("{} {}", flavor.flavor, catalog_release.release.tag_name)
                || installed_tool.internal_name
                    == format!("{}{}", flavor.flavor, catalog_release.release.tag_name)
            {
                return Some(catalog_release.clone());
            }

            None
        })
    })
}

fn proton_ge_tool_name_matches_release(tool_name: &str, release_tag: &str) -> bool {
    tool_name == release_tag
        || tool_name.strip_suffix("-x86_64") == Some(release_tag)
        || tool_name.strip_suffix("-aarch64") == Some(release_tag)
}

fn proton_cachyos_tool_name_matches_release(tool_name: &str, release_tag: &str) -> bool {
    let normalized_tag = normalize_release_identity(release_tag);
    let mut normalized_tool = normalize_release_identity(tool_name);
    for architecture_suffix in ["x8664v3", "x8664v2", "x8664", "aarch64"] {
        if let Some(without_suffix) = normalized_tool.strip_suffix(architecture_suffix) {
            normalized_tool = without_suffix.to_string();
            break;
        }
    }

    normalized_tool == normalized_tag || normalized_tool == format!("proton{}", normalized_tag)
}

fn normalize_release_identity(value: &str) -> String {
    value
        .chars()
        .flat_map(char::to_lowercase)
        .filter(|character| character.is_ascii_alphanumeric())
        .collect()
}

fn apply_catalog_release(
    installed_tool: &mut InstalledCompatibilityTool,
    catalog_release: &CatalogRelease,
) {
    installed_tool.flavor = catalog_release.flavor.clone();
    installed_tool.catalog_release_id = Some(catalog_release.id.clone());
    installed_tool.github_release = Some(catalog_release.release.clone());
}

fn operation_targets_installed_tool(operation: &OperationInfo, installed_tool_id: &str) -> bool {
    operation.installed_tool_id.as_deref() == Some(installed_tool_id)
}

fn should_skip_operation_broadcast(
    last_broadcast: Option<&(OperationStateSnapshot, Instant)>,
    next_snapshot: &OperationStateSnapshot,
    now: Instant,
) -> bool {
    let Some((last_snapshot, last_sent_at)) = last_broadcast else {
        return false;
    };

    if last_snapshot == next_snapshot {
        return true;
    }

    is_download_progress_update_throttled(
        last_snapshot,
        next_snapshot,
        now.duration_since(*last_sent_at),
    )
}

fn is_download_progress_update_throttled(
    last_snapshot: &OperationStateSnapshot,
    next_snapshot: &OperationStateSnapshot,
    elapsed_since_last_broadcast: Duration,
) -> bool {
    if elapsed_since_last_broadcast >= DOWNLOAD_PROGRESS_BROADCAST_INTERVAL {
        return false;
    }

    if last_snapshot.queued_operations != next_snapshot.queued_operations {
        return false;
    }

    let (Some(last_operation), Some(next_operation)) = (
        last_snapshot.current_operation.as_ref(),
        next_snapshot.current_operation.as_ref(),
    ) else {
        return false;
    };

    last_operation.id == next_operation.id
        && last_operation.kind == next_operation.kind
        && last_operation.state == OperationState::Downloading
        && next_operation.state == OperationState::Downloading
        && next_operation.progress < 100
        && last_operation.label == next_operation.label
        && last_operation.release_id == next_operation.release_id
        && last_operation.installed_tool_id == next_operation.installed_tool_id
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::release_util::Release;
    use crate::wine_cask::generate_compatibility_tool_vdf;
    use std::fs;
    use std::path::PathBuf;

    fn operation(state: OperationState, progress: u8) -> OperationInfo {
        OperationInfo {
            id: "operation-1".to_string(),
            label: message("operation-install", &[("tool", "GE-Proton")]),
            kind: OperationKind::Install,
            state,
            progress,
            download: None,
            release_id: Some("release-1".to_string()),
            installed_tool_id: None,
        }
    }

    fn snapshot(state: OperationState, progress: u8) -> OperationStateSnapshot {
        OperationStateSnapshot {
            current_operation: Some(operation(state, progress)),
            queued_operations: Vec::new(),
        }
    }

    fn mapping_test_app(
        steam_path: PathBuf,
        mappings: HashMap<u64, String>,
        mappings_stale: bool,
    ) -> WineCask {
        WineCask {
            steam_util: SteamUtil::new(steam_path),
            app_state: Arc::new(Mutex::new(AppState {
                catalog_flavors: Vec::new(),
                installed_tools: Vec::new(),
                app_compat_tool_mappings: mappings,
                app_compat_tool_mappings_stale: mappings_stale,
                current_operation: None,
                queued_operations: Vec::new(),
                updater_state: UpdaterState::Idle,
                updater_last_check: None,
                steam_visible_tools: Vec::new(),
                operation_queue: VecDeque::new(),
                app_compat_tool_mappings_last_refresh_attempt: None,
                app_compat_tool_mappings_refresh_in_progress: false,
            })),
            operation_broadcast_cache: Arc::new(Mutex::new(None)),
            queue_notify: Arc::new(Notify::new()),
        }
    }

    async fn catalog_test_app(steam_path: PathBuf) -> WineCask {
        let app = mapping_test_app(steam_path, HashMap::new(), false);
        app.app_state.lock().await.catalog_flavors = vec![Flavor {
            flavor: CompatibilityToolFlavor::ProtonGE,
            releases: vec![CatalogRelease {
                id: "release-1".to_string(),
                flavor: CompatibilityToolFlavor::ProtonGE,
                release: Release {
                    id: 1,
                    tag_name: "GE-Proton11-6".to_string(),
                    name: "GE-Proton11-6".to_string(),
                    url: String::new(),
                    draft: false,
                    prerelease: false,
                    assets: Vec::new(),
                    created_at: String::new(),
                    published_at: String::new(),
                    tarball_url: String::new(),
                    body: String::new(),
                },
            }],
        }];
        app
    }

    #[tokio::test]
    async fn install_queue_rejects_duplicates_and_can_retry_after_cancellation() {
        let directory = tempfile::tempdir().unwrap();
        let app = catalog_test_app(directory.path().to_path_buf()).await;
        let peers = PeerMap::new(Mutex::new(HashMap::new()));
        let command: Command = serde_json::from_str(
            r#"{"type":"InstallCatalogRelease","release_id":"release-1"}"#,
        )
        .unwrap();
        let Command::InstallCatalogRelease { release_id } = command else {
            panic!("Expected an install command");
        };

        for _ in 0..2 {
            app.queue_install_catalog_release(release_id.clone(), &peers).await;
        }
        assert_eq!(app.app_state.lock().await.operation_queue.len(), 1);
        assert_eq!(app.app_state.lock().await.queued_operations.len(), 1);

        let active = app.begin_next_operation(&peers).await.unwrap();
        app.queue_install_catalog_release(release_id.clone(), &peers).await;
        assert!(app.app_state.lock().await.operation_queue.is_empty());
        app.cancel_operation(active.operation.id, &peers).await;
        app.queue_install_catalog_release(release_id.clone(), &peers).await;
        assert!(app.app_state.lock().await.operation_queue.is_empty());
        app.complete_current_operation(&peers).await;

        app.queue_install_catalog_release(release_id, &peers).await;
        let queued_id = app.app_state.lock().await.queued_operations[0].id.clone();
        app.cancel_operation(queued_id, &peers).await;
        let state = app.app_state.lock().await;
        assert!(state.operation_queue.is_empty());
        assert!(state.queued_operations.is_empty());
    }

    #[tokio::test]
    async fn installed_release_can_be_queued_again_after_removal() {
        let directory = tempfile::tempdir().unwrap();
        let tool_path = directory.path().join("compatibilitytools.d/GE-Proton11-6-x86_64");
        fs::create_dir_all(&tool_path).unwrap();
        generate_compatibility_tool_vdf(
            tool_path.join("compatibilitytool.vdf"),
            "GE-Proton11-6-x86_64",
            "GE-Proton11-6",
        ).unwrap();
        let app = catalog_test_app(directory.path().to_path_buf()).await;
        let peers = PeerMap::new(Mutex::new(HashMap::new()));
        app.sync_backend_state().await;
        let installed = app.app_state.lock().await.installed_tools[0].clone();
        assert_eq!(installed.catalog_release_id.as_deref(), Some("release-1"));
        assert!(installed.requires_restart);
        app.process_frontend_compat_tools_update(&peers, vec![SteamClientCompatToolInfo {
            str_tool_name: installed.internal_name.clone(),
            str_display_name: installed.display_name.clone(),
        }]).await;
        assert!(!app.app_state.lock().await.installed_tools[0].requires_restart);

        app.queue_install_catalog_release("release-1".to_string(), &peers).await;
        app.queue_install_catalog_release("unknown".to_string(), &peers).await;
        assert!(app.app_state.lock().await.operation_queue.is_empty());
        for _ in 0..2 {
            app.queue_uninstall_installed_tool(installed.id.clone(), &peers).await;
        }
        assert_eq!(app.app_state.lock().await.operation_queue.len(), 1);
        app.begin_next_operation(&peers).await.unwrap();
        app.queue_uninstall_installed_tool(installed.id.clone(), &peers).await;
        assert!(app.app_state.lock().await.operation_queue.is_empty());
        app.uninstall_installed_tool(installed.id, &peers).await;
        app.complete_current_operation(&peers).await;
        assert!(!tool_path.exists());
        assert!(app.app_state.lock().await.installed_tools.is_empty());

        app.queue_install_catalog_release("release-1".to_string(), &peers).await;
        assert_eq!(app.app_state.lock().await.operation_queue.len(), 1);
    }

    #[test]
    fn throttles_download_progress_until_interval_elapses() {
        let now = Instant::now();
        let last_broadcast = (snapshot(OperationState::Downloading, 37), now);
        let next_snapshot = snapshot(OperationState::Downloading, 38);

        assert!(should_skip_operation_broadcast(
            Some(&last_broadcast),
            &next_snapshot,
            now + Duration::from_millis(50),
        ));
        assert!(!should_skip_operation_broadcast(
            Some(&last_broadcast),
            &next_snapshot,
            now + DOWNLOAD_PROGRESS_BROADCAST_INTERVAL,
        ));
    }

    #[test]
    fn throttles_download_statistics_when_percentage_has_not_changed() {
        let now = Instant::now();
        let last_broadcast = (snapshot(OperationState::Downloading, 37), now);
        let mut next_snapshot = snapshot(OperationState::Downloading, 37);
        next_snapshot.current_operation.as_mut().unwrap().download = Some(DownloadProgress {
            bytes_downloaded: 370,
            total_bytes: Some(1000),
            bytes_per_second: Some(100),
            eta_seconds: Some(7),
            elapsed_seconds: 4,
        });

        assert!(should_skip_operation_broadcast(
            Some(&last_broadcast),
            &next_snapshot,
            now + Duration::from_millis(50),
        ));
        assert!(!should_skip_operation_broadcast(
            Some(&last_broadcast),
            &next_snapshot,
            now + DOWNLOAD_PROGRESS_BROADCAST_INTERVAL,
        ));
    }

    #[tokio::test]
    async fn compatibility_mapping_refresh_retains_last_good_state_on_read_error() {
        let directory = tempfile::tempdir().unwrap();
        let existing_mappings = HashMap::from([(123_u64, "known-tool".to_string())]);
        let app = mapping_test_app(
            directory.path().to_path_buf(),
            existing_mappings.clone(),
            false,
        );

        app.refresh_app_compat_tool_mappings().await;

        let app_state = app.app_state.lock().await;
        assert_eq!(app_state.app_compat_tool_mappings, existing_mappings);
        assert!(app_state.app_compat_tool_mappings_stale);
        assert!(!app_state.app_compat_tool_mappings_refresh_in_progress);
    }

    #[tokio::test]
    async fn compatibility_mapping_refresh_clears_stale_state_on_success() {
        let directory = tempfile::tempdir().unwrap();
        let config_directory = directory.path().join("config");
        std::fs::create_dir(&config_directory).unwrap();
        std::fs::write(
            config_directory.join("config.vdf"),
            r#""InstallConfigStore"
            {
                "Software"
                {
                    "Valve"
                    {
                        "Steam"
                        {
                            "CompatToolMapping"
                            {
                                "123"
                                {
                                    "name" "forced-tool"
                                    "priority" "250"
                                }
                            }
                        }
                    }
                }
            }"#,
        )
        .unwrap();
        let app = mapping_test_app(directory.path().to_path_buf(), HashMap::new(), true);

        app.refresh_app_compat_tool_mappings().await;

        let app_state = app.app_state.lock().await;
        assert_eq!(
            app_state.app_compat_tool_mappings.get(&123),
            Some(&"forced-tool".to_string())
        );
        assert!(!app_state.app_compat_tool_mappings_stale);
        assert!(!app_state.app_compat_tool_mappings_refresh_in_progress);
    }

    #[tokio::test]
    async fn compatibility_mapping_refresh_throttles_rapid_attempts() {
        let directory = tempfile::tempdir().unwrap();
        let app = mapping_test_app(directory.path().to_path_buf(), HashMap::new(), false);

        app.refresh_app_compat_tool_mappings().await;
        let first_attempt = app
            .app_state
            .lock()
            .await
            .app_compat_tool_mappings_last_refresh_attempt;

        // Use the stale bit as a sentinel: another failed read would set it back to true.
        app.app_state.lock().await.app_compat_tool_mappings_stale = false;
        app.refresh_app_compat_tool_mappings().await;

        let app_state = app.app_state.lock().await;
        assert_eq!(
            app_state.app_compat_tool_mappings_last_refresh_attempt,
            first_attempt
        );
        assert!(!app_state.app_compat_tool_mappings_stale);
    }

    #[tokio::test]
    async fn download_statistics_clear_on_extraction_and_cancellation() {
        let directory = tempfile::tempdir().unwrap();
        let app = WineCask {
            steam_util: SteamUtil::new(directory.path().to_path_buf()),
            app_state: Arc::new(Mutex::new(AppState {
                catalog_flavors: Vec::new(),
                installed_tools: Vec::new(),
                app_compat_tool_mappings: HashMap::new(),
                app_compat_tool_mappings_stale: true,
                current_operation: Some(operation(OperationState::Downloading, 0)),
                queued_operations: Vec::new(),
                updater_state: UpdaterState::Idle,
                updater_last_check: None,
                steam_visible_tools: Vec::new(),
                operation_queue: VecDeque::new(),
                app_compat_tool_mappings_last_refresh_attempt: None,
                app_compat_tool_mappings_refresh_in_progress: false,
            })),
            operation_broadcast_cache: Arc::new(Mutex::new(None)),
            queue_notify: Arc::new(Notify::new()),
        };
        let peers = PeerMap::new(Mutex::new(HashMap::new()));
        let download = DownloadProgress {
            bytes_downloaded: 500,
            total_bytes: Some(1000),
            bytes_per_second: Some(100),
            eta_seconds: Some(5),
            elapsed_seconds: 5,
        };
        app.update_current_download(download.clone(), &peers).await;
        {
            let state = app.app_state.lock().await;
            let current = state.current_operation.as_ref().unwrap();
            assert_eq!(current.progress, 50);
            let serialized = serde_json::to_value(current).unwrap();
            assert_eq!(serialized["download"]["bytes_per_second"], 100);
        }

        app.update_current_operation(OperationState::Extracting, 0, &peers)
            .await;
        assert!(app
            .app_state
            .lock()
            .await
            .current_operation
            .as_ref()
            .unwrap()
            .download
            .is_none());

        app.update_current_download(download.clone(), &peers).await;
        app.cancel_operation("operation-1".to_string(), &peers)
            .await;
        app.update_current_download(download, &peers).await;
        let state = app.app_state.lock().await;
        let current = state.current_operation.as_ref().unwrap();
        assert!(current.state == OperationState::Cancelling);
        assert!(current.download.is_none());
    }

    #[test]
    fn allows_immediate_state_transitions() {
        let now = Instant::now();
        let last_broadcast = (snapshot(OperationState::Downloading, 99), now);
        let next_snapshot = snapshot(OperationState::Extracting, 0);

        assert!(!should_skip_operation_broadcast(
            Some(&last_broadcast),
            &next_snapshot,
            now + Duration::from_millis(50),
        ));
    }

    #[test]
    fn allows_completion_progress_immediately() {
        let now = Instant::now();
        let last_broadcast = (snapshot(OperationState::Downloading, 99), now);
        let next_snapshot = snapshot(OperationState::Downloading, 100);

        assert!(!should_skip_operation_broadcast(
            Some(&last_broadcast),
            &next_snapshot,
            now + Duration::from_millis(50),
        ));
    }

    #[test]
    fn matches_proton_ge_architecture_suffixes() {
        assert!(proton_ge_tool_name_matches_release(
            "GE-Proton11-6-x86_64",
            "GE-Proton11-6"
        ));
        assert!(proton_ge_tool_name_matches_release(
            "GE-Proton11-6-aarch64",
            "GE-Proton11-6"
        ));
        assert!(proton_ge_tool_name_matches_release(
            "GE-Proton10-34",
            "GE-Proton10-34"
        ));
    }

    #[test]
    fn does_not_match_unrelated_proton_ge_suffixes_or_versions() {
        assert!(!proton_ge_tool_name_matches_release(
            "GE-Proton11-6-hotfix",
            "GE-Proton11-6"
        ));
        assert!(!proton_ge_tool_name_matches_release(
            "GE-Proton11-60-x86_64",
            "GE-Proton11-6"
        ));
    }

    #[test]
    fn matches_exact_cachyos_release_identity_with_architecture_suffix() {
        assert!(proton_cachyos_tool_name_matches_release(
            "proton-cachyos-11.0-20260703-slr-x86_64_v3",
            "cachyos-11.0-20260703-slr"
        ));
        assert!(proton_cachyos_tool_name_matches_release(
            "CachyOS 11.0-20260703 SLR",
            "cachyos-11.0-20260703-slr"
        ));
    }

    #[test]
    fn rejects_ambiguous_or_different_cachyos_release_identity() {
        assert!(!proton_cachyos_tool_name_matches_release(
            "Proton CachyOS",
            "cachyos-11.0-20260703-slr"
        ));
        assert!(!proton_cachyos_tool_name_matches_release(
            "proton-cachyos-11.0-20260702-slr-x86_64_v3",
            "cachyos-11.0-20260703-slr"
        ));
    }

}

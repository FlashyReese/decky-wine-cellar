use crate::steam_util::SteamUtil;
use crate::wine_cask::flavors::{
    CatalogRelease, CompatibilityToolFlavor, Flavor, InstalledCompatibilityTool,
    InstalledToolSource, SteamClientCompatToolInfo, VirtualCompatibilityTool,
};
use crate::wine_cask::virtual_tools::normalize_virtual_tool_label;
use crate::PeerMap;
use log::{debug, error, info, warn};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::{Mutex, Notify};
use tokio_tungstenite::tungstenite::Message;

const DOWNLOAD_PROGRESS_BROADCAST_INTERVAL: Duration = Duration::from_millis(200);

pub struct WineCask {
    pub steam_util: SteamUtil,
    pub app_state: Arc<Mutex<AppState>>,
    pub operation_broadcast_cache: Arc<Mutex<Option<(OperationStateSnapshot, Instant)>>>,
    pub queue_notify: Arc<Notify>,
    pub virtual_tool_manifest_path: PathBuf,
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
    pub virtual_tools: Vec<VirtualCompatibilityTool>,
    pub current_operation: Option<OperationInfo>,
    pub queued_operations: Vec<OperationInfo>,
    pub updater_state: UpdaterState,
    pub updater_last_check: Option<u64>,
    #[serde(skip)]
    pub steam_visible_tools: Vec<SteamClientCompatToolInfo>,
    #[serde(skip)]
    pub operation_queue: VecDeque<QueuedCommand>,
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
    pub notification: Option<String>,
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
        target: InstallTarget,
    },
    UninstallInstalledTool {
        installed_tool_id: String,
    },
    CancelOperation {
        operation_id: String,
    },
    CreateVirtualTool {
        user_label: String,
    },
    RenameVirtualTool {
        virtual_tool_id: String,
        user_label: String,
    },
    RemoveVirtualTool {
        virtual_tool_id: String,
    },
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
#[serde(tag = "type")]
pub enum InstallTarget {
    Direct,
    VirtualTool { virtual_tool_id: String },
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub enum OperationKind {
    Install,
    Uninstall,
    CreateVirtualTool,
    RenameVirtualTool,
    RemoveVirtualTool,
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
    pub label: String,
    pub kind: OperationKind,
    pub state: OperationState,
    pub progress: u8,
    pub release_id: Option<String>,
    pub installed_tool_id: Option<String>,
    pub virtual_tool_id: Option<String>,
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
        target: InstallTarget,
        peer_map: &PeerMap,
    ) {
        let Some(catalog_release) = self.get_catalog_release(&release_id).await else {
            self.broadcast_notification(peer_map, "Unknown release requested")
                .await;
            return;
        };

        let (label, virtual_tool_id) = match &target {
            InstallTarget::Direct => (
                format!("Install {}", catalog_release.release.tag_name),
                None,
            ),
            InstallTarget::VirtualTool { virtual_tool_id } => {
                let Some(virtual_tool) = self.get_virtual_tool(virtual_tool_id).await else {
                    self.broadcast_notification(peer_map, "Unknown virtual tool requested")
                        .await;
                    return;
                };
                (
                    format!(
                        "Mount {} to {}",
                        catalog_release.release.tag_name, virtual_tool.user_label
                    ),
                    Some(virtual_tool_id.clone()),
                )
            }
        };

        let mut app_state = self.app_state.lock().await;
        if matches!(&target, InstallTarget::Direct)
            && app_state.installed_tools.iter().any(|tool| {
                matches!(tool.source, InstalledToolSource::Direct)
                    && tool.catalog_release_id.as_deref() == Some(catalog_release.id.as_str())
            })
        {
            drop(app_state);
            self.broadcast_notification(peer_map, "That release is already installed")
                .await;
            return;
        }

        if app_state
            .current_operation
            .as_ref()
            .map(|operation| install_operation_matches_target(operation, &release_id, &target))
            .unwrap_or(false)
            || app_state.operation_queue.iter().any(|queued| {
                install_operation_matches_target(&queued.operation, &release_id, &target)
            })
        {
            drop(app_state);
            self.broadcast_notification(peer_map, duplicate_install_notification_message(&target))
                .await;
            return;
        }

        let queued_command = QueuedCommand {
            command: Command::InstallCatalogRelease { release_id, target },
            operation: OperationInfo {
                id: operation_id(),
                label,
                kind: OperationKind::Install,
                state: OperationState::Pending,
                progress: 0,
                release_id: Some(catalog_release.id),
                installed_tool_id: None,
                virtual_tool_id,
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
            self.broadcast_notification(peer_map, "Unknown installed tool requested")
                .await;
            return;
        };

        let label = installed_tool
            .user_label
            .clone()
            .unwrap_or_else(|| installed_tool.display_name.clone());

        let app_state = self.app_state.lock().await;
        if app_state
            .current_operation
            .as_ref()
            .map(|operation| {
                operation_targets_installed_tool(operation, &installed_tool_id)
                    || installed_tool
                        .virtual_tool_id
                        .as_deref()
                        .map(|virtual_tool_id| {
                            operation_targets_virtual_tool(operation, virtual_tool_id)
                        })
                        .unwrap_or(false)
            })
            .unwrap_or(false)
            || app_state.operation_queue.iter().any(|queued| {
                operation_targets_installed_tool(&queued.operation, &installed_tool_id)
                    || installed_tool
                        .virtual_tool_id
                        .as_deref()
                        .map(|virtual_tool_id| {
                            operation_targets_virtual_tool(&queued.operation, virtual_tool_id)
                        })
                        .unwrap_or(false)
            })
        {
            drop(app_state);
            self.broadcast_notification(
                peer_map,
                "That compatibility tool already has an active or queued operation",
            )
            .await;
            return;
        }
        drop(app_state);

        let queued_command = QueuedCommand {
            command: Command::UninstallInstalledTool { installed_tool_id },
            operation: OperationInfo {
                id: operation_id(),
                label: format!("Remove {}", label),
                kind: OperationKind::Uninstall,
                state: OperationState::Pending,
                progress: 0,
                release_id: installed_tool.catalog_release_id.clone(),
                installed_tool_id: Some(installed_tool.id.clone()),
                virtual_tool_id: installed_tool.virtual_tool_id.clone(),
            },
        };

        self.app_state
            .lock()
            .await
            .operation_queue
            .push_back(queued_command);
        self.sync_public_queue_snapshot().await;
        self.queue_notify.notify_one();
        self.broadcast_operation_state(peer_map).await;
    }

    pub async fn queue_create_virtual_tool(&self, user_label: String, peer_map: &PeerMap) {
        let trimmed_label = match normalize_virtual_tool_label(&user_label) {
            Ok(label) => label,
            Err(err) => {
                self.broadcast_notification(peer_map, &err).await;
                return;
            }
        };

        let queued_command = QueuedCommand {
            command: Command::CreateVirtualTool {
                user_label: trimmed_label.clone(),
            },
            operation: OperationInfo {
                id: operation_id(),
                label: format!("Create {}", trimmed_label),
                kind: OperationKind::CreateVirtualTool,
                state: OperationState::Pending,
                progress: 0,
                release_id: None,
                installed_tool_id: None,
                virtual_tool_id: None,
            },
        };

        self.app_state
            .lock()
            .await
            .operation_queue
            .push_back(queued_command);
        self.sync_public_queue_snapshot().await;
        self.queue_notify.notify_one();
        self.broadcast_operation_state(peer_map).await;
    }

    pub async fn queue_rename_virtual_tool(
        &self,
        virtual_tool_id: String,
        user_label: String,
        peer_map: &PeerMap,
    ) {
        let trimmed_label = match normalize_virtual_tool_label(&user_label) {
            Ok(label) => label,
            Err(err) => {
                self.broadcast_notification(peer_map, &err).await;
                return;
            }
        };

        let Some(virtual_tool) = self.get_virtual_tool(&virtual_tool_id).await else {
            self.broadcast_notification(peer_map, "Unknown virtual tool requested")
                .await;
            return;
        };

        let app_state = self.app_state.lock().await;
        if app_state
            .current_operation
            .as_ref()
            .map(|operation| operation_targets_virtual_tool(operation, &virtual_tool_id))
            .unwrap_or(false)
            || app_state
                .operation_queue
                .iter()
                .any(|queued| operation_targets_virtual_tool(&queued.operation, &virtual_tool_id))
        {
            drop(app_state);
            self.broadcast_notification(
                peer_map,
                "That virtual tool already has an active or queued operation",
            )
            .await;
            return;
        }
        drop(app_state);

        let queued_command = QueuedCommand {
            command: Command::RenameVirtualTool {
                virtual_tool_id: virtual_tool_id.clone(),
                user_label: trimmed_label.clone(),
            },
            operation: OperationInfo {
                id: operation_id(),
                label: format!("Rename {} to {}", virtual_tool.user_label, trimmed_label),
                kind: OperationKind::RenameVirtualTool,
                state: OperationState::Pending,
                progress: 0,
                release_id: None,
                installed_tool_id: virtual_tool.installed_tool_id.clone(),
                virtual_tool_id: Some(virtual_tool_id),
            },
        };

        self.app_state
            .lock()
            .await
            .operation_queue
            .push_back(queued_command);
        self.sync_public_queue_snapshot().await;
        self.queue_notify.notify_one();
        self.broadcast_operation_state(peer_map).await;
    }

    pub async fn queue_remove_virtual_tool(&self, virtual_tool_id: String, peer_map: &PeerMap) {
        let Some(virtual_tool) = self.get_virtual_tool(&virtual_tool_id).await else {
            self.broadcast_notification(peer_map, "Unknown virtual tool requested")
                .await;
            return;
        };

        let app_state = self.app_state.lock().await;
        if app_state
            .current_operation
            .as_ref()
            .map(|operation| operation_targets_virtual_tool(operation, &virtual_tool_id))
            .unwrap_or(false)
            || app_state
                .operation_queue
                .iter()
                .any(|queued| operation_targets_virtual_tool(&queued.operation, &virtual_tool_id))
        {
            drop(app_state);
            self.broadcast_notification(
                peer_map,
                "That virtual tool already has an active or queued operation",
            )
            .await;
            return;
        }
        drop(app_state);

        let queued_command = QueuedCommand {
            command: Command::RemoveVirtualTool {
                virtual_tool_id: virtual_tool_id.clone(),
            },
            operation: OperationInfo {
                id: operation_id(),
                label: format!("Remove {}", virtual_tool.user_label),
                kind: OperationKind::RemoveVirtualTool,
                state: OperationState::Pending,
                progress: 0,
                release_id: virtual_tool.current_payload_release_id.clone(),
                installed_tool_id: virtual_tool.installed_tool_id.clone(),
                virtual_tool_id: Some(virtual_tool_id),
            },
        };

        self.app_state
            .lock()
            .await
            .operation_queue
            .push_back(queued_command);
        self.sync_public_queue_snapshot().await;
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
            self.broadcast_notification(peer_map, "Cancelled queued operation")
                .await;
            return;
        }

        if let Some(operation) = &mut app_state.current_operation {
            if operation.id == operation_id {
                if operation.kind == OperationKind::Install {
                    operation.state = OperationState::Cancelling;
                    drop(app_state);
                    self.broadcast_operation_state(peer_map).await;
                    self.broadcast_notification(peer_map, "Cancelling install")
                        .await;
                    return;
                }

                drop(app_state);
                self.broadcast_notification(
                    peer_map,
                    "Only active installs can be cancelled once they start",
                )
                .await;
                return;
            }
        }

        drop(app_state);
        self.broadcast_notification(peer_map, "Operation not found")
            .await;
    }

    async fn sync_public_queue_snapshot(&self) {
        let mut app_state = self.app_state.lock().await;
        app_state.queued_operations = app_state
            .operation_queue
            .iter()
            .map(|queued| queued.operation.clone())
            .collect();
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

    pub async fn broadcast_notification(&self, peer_map: &PeerMap, message: &str) {
        let response_new = MessageEnvelope {
            r#type: MessageType::Notification,
            command: None,
            notification: Some(message.to_string()),
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
                source: InstalledToolSource::Direct,
                virtual_tool_id: None,
                user_label: None,
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

    pub async fn sync_backend_state(&self) {
        let (catalog_flavors, steam_visible_tools) = {
            let app_state = self.app_state.lock().await;
            (
                app_state.catalog_flavors.clone(),
                app_state.steam_visible_tools.clone(),
            )
        };

        let virtual_manifest = self.load_virtual_tool_manifest();
        let visible_tool_names: HashSet<String> = steam_visible_tools
            .iter()
            .map(|tool| tool.str_tool_name.clone())
            .collect();
        let catalog_lookup = build_catalog_lookup(&catalog_flavors);

        let mut installed_tools = self.list_compatibility_tools().unwrap_or_default();
        for installed_tool in &mut installed_tools {
            installed_tool.requires_restart =
                !visible_tool_names.contains(&installed_tool.internal_name);

            if let Some(virtual_tool_config) = virtual_manifest.tools.iter().find(|config| {
                config.directory_name == installed_tool.directory_name
                    || config.steam_internal_name == installed_tool.internal_name
            }) {
                installed_tool.source = InstalledToolSource::Virtual;
                installed_tool.virtual_tool_id = Some(virtual_tool_config.id.clone());
                installed_tool.user_label = Some(virtual_tool_config.user_label.clone());
            }

            if let Some(virtual_tool_id) = &installed_tool.virtual_tool_id {
                if let Some(virtual_tool_config) = virtual_manifest
                    .tools
                    .iter()
                    .find(|config| &config.id == virtual_tool_id)
                {
                    if let Some(release_id) = &virtual_tool_config.current_payload_release_id {
                        if let Some(catalog_release) = catalog_lookup.get(release_id) {
                            apply_catalog_release(installed_tool, catalog_release);
                        }
                    }
                }
                continue;
            }

            if let Some(catalog_release) =
                find_catalog_release_for_tool(&catalog_flavors, installed_tool)
            {
                apply_catalog_release(installed_tool, &catalog_release);
            }
        }

        let virtual_tools = virtual_manifest
            .tools
            .iter()
            .map(|virtual_tool_config| {
                let installed_tool = installed_tools.iter().find(|tool| {
                    tool.virtual_tool_id.as_deref() == Some(virtual_tool_config.id.as_str())
                });
                let current_payload_release = virtual_tool_config
                    .current_payload_release_id
                    .as_ref()
                    .and_then(|release_id| catalog_lookup.get(release_id));
                let github_release =
                    current_payload_release.map(|catalog_release| catalog_release.release.clone());
                let current_payload_name = github_release
                    .as_ref()
                    .map(|release| release.tag_name.clone());
                let current_payload_flavor = current_payload_release
                    .map(|catalog_release| catalog_release.flavor.clone())
                    .unwrap_or(CompatibilityToolFlavor::Unknown);

                VirtualCompatibilityTool {
                    id: virtual_tool_config.id.clone(),
                    user_label: virtual_tool_config.user_label.clone(),
                    steam_internal_name: virtual_tool_config.steam_internal_name.clone(),
                    directory_name: virtual_tool_config.directory_name.clone(),
                    installed_tool_id: installed_tool.map(|tool| tool.id.clone()),
                    current_payload_release_id: virtual_tool_config
                        .current_payload_release_id
                        .clone(),
                    current_payload_name,
                    current_payload_flavor,
                    github_release,
                    requires_restart: installed_tool
                        .map(|tool| tool.requires_restart)
                        .unwrap_or(true),
                    used_by_games: installed_tool
                        .map(|tool| tool.used_by_games.clone())
                        .unwrap_or_default(),
                }
            })
            .collect();

        let mut app_state = self.app_state.lock().await;
        app_state.installed_tools = installed_tools;
        app_state.virtual_tools = virtual_tools;
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
        let is_empty = {
            let app_state = self.app_state.lock().await;
            app_state.catalog_flavors.is_empty()
        };
        if is_empty {
            let flavors = self.get_flavors(false).await;
            let mut app_state = self.app_state.lock().await;
            app_state.catalog_flavors = flavors;
        }

        self.app_state
            .lock()
            .await
            .catalog_flavors
            .iter()
            .flat_map(|flavor| flavor.releases.iter())
            .find(|release| release.id == release_id)
            .cloned()
    }

    pub async fn reclaim_memory_if_idle(&self, peer_map: &PeerMap) {
        let is_empty = peer_map.lock().await.is_empty();
        if is_empty {
            let can_clear = {
                let app_state = self.app_state.lock().await;
                app_state.current_operation.is_none() && app_state.operation_queue.is_empty()
            };
            if can_clear {
                info!("All clients disconnected and no active operations. Clearing catalog flavors.");
                let mut app_state = self.app_state.lock().await;
                app_state.catalog_flavors.clear();
                drop(app_state);

                let mut cache = self.operation_broadcast_cache.lock().await;
                *cache = None;
                drop(cache);
            }
        }

        #[cfg(target_os = "linux")]
        {
            info!("Calling malloc_trim to release memory to OS");
            unsafe {
                libc::malloc_trim(0);
            }
        }
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

    pub async fn get_virtual_tool(
        &self,
        virtual_tool_id: &str,
    ) -> Option<VirtualCompatibilityTool> {
        self.app_state
            .lock()
            .await
            .virtual_tools
            .iter()
            .find(|tool| tool.id == virtual_tool_id)
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

fn build_catalog_lookup(catalog_flavors: &[Flavor]) -> HashMap<String, CatalogRelease> {
    catalog_flavors
        .iter()
        .flat_map(|flavor| flavor.releases.iter())
        .map(|release| (release.id.clone(), release.clone()))
        .collect()
}

fn find_catalog_release_for_tool(
    catalog_flavors: &[Flavor],
    installed_tool: &InstalledCompatibilityTool,
) -> Option<CatalogRelease> {
    catalog_flavors.iter().find_map(|flavor| {
        flavor.releases.iter().find_map(|catalog_release| {
            if flavor.flavor == CompatibilityToolFlavor::ProtonGE {
                if installed_tool.internal_name == catalog_release.release.tag_name
                    || installed_tool.display_name == catalog_release.release.tag_name
                {
                    return Some(catalog_release.clone());
                }
            } else if flavor.flavor == CompatibilityToolFlavor::ProtonCachyOS {
                if installed_tool
                    .display_name
                    .to_lowercase()
                    .contains("cachyos")
                {
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

fn apply_catalog_release(
    installed_tool: &mut InstalledCompatibilityTool,
    catalog_release: &CatalogRelease,
) {
    installed_tool.flavor = catalog_release.flavor.clone();
    installed_tool.catalog_release_id = Some(catalog_release.id.clone());
    installed_tool.github_release = Some(catalog_release.release.clone());
}

fn install_operation_matches_target(
    operation: &OperationInfo,
    release_id: &str,
    target: &InstallTarget,
) -> bool {
    operation.kind == OperationKind::Install
        && operation.release_id.as_deref() == Some(release_id)
        && match target {
            InstallTarget::Direct => operation.virtual_tool_id.is_none(),
            InstallTarget::VirtualTool { virtual_tool_id } => {
                operation.virtual_tool_id.as_deref() == Some(virtual_tool_id)
            }
        }
}

fn duplicate_install_notification_message(target: &InstallTarget) -> &'static str {
    match target {
        InstallTarget::Direct => "That release is already queued or installing",
        InstallTarget::VirtualTool { .. } => "That release is already queued for that virtual tool",
    }
}

fn operation_targets_virtual_tool(operation: &OperationInfo, virtual_tool_id: &str) -> bool {
    operation.virtual_tool_id.as_deref() == Some(virtual_tool_id)
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
        && last_operation.progress != next_operation.progress
        && last_operation.label == next_operation.label
        && last_operation.release_id == next_operation.release_id
        && last_operation.installed_tool_id == next_operation.installed_tool_id
        && last_operation.virtual_tool_id == next_operation.virtual_tool_id
}

#[cfg(test)]
mod tests {
    use super::*;

    fn operation(state: OperationState, progress: u8) -> OperationInfo {
        OperationInfo {
            id: "operation-1".to_string(),
            label: "Install GE-Proton".to_string(),
            kind: OperationKind::Install,
            state,
            progress,
            release_id: Some("release-1".to_string()),
            installed_tool_id: None,
            virtual_tool_id: None,
        }
    }

    fn snapshot(state: OperationState, progress: u8) -> OperationStateSnapshot {
        OperationStateSnapshot {
            current_operation: Some(operation(state, progress)),
            queued_operations: Vec::new(),
        }
    }

    #[test]
    fn skips_identical_operation_snapshots() {
        let now = Instant::now();
        let current_snapshot = snapshot(OperationState::Downloading, 37);
        let last_broadcast = (current_snapshot.clone(), now);

        assert!(should_skip_operation_broadcast(
            Some(&last_broadcast),
            &current_snapshot,
            now + Duration::from_millis(50),
        ));
    }

    #[test]
    fn throttles_rapid_download_progress_updates() {
        let now = Instant::now();
        let last_broadcast = (snapshot(OperationState::Downloading, 37), now);
        let next_snapshot = snapshot(OperationState::Downloading, 38);

        assert!(should_skip_operation_broadcast(
            Some(&last_broadcast),
            &next_snapshot,
            now + Duration::from_millis(50),
        ));
    }

    #[test]
    fn allows_download_progress_updates_after_throttle_window() {
        let now = Instant::now();
        let last_broadcast = (snapshot(OperationState::Downloading, 37), now);
        let next_snapshot = snapshot(OperationState::Downloading, 38);

        assert!(!should_skip_operation_broadcast(
            Some(&last_broadcast),
            &next_snapshot,
            now + DOWNLOAD_PROGRESS_BROADCAST_INTERVAL,
        ));
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
}

use crate::steam_util::SteamUtil;
use crate::wine_cask::flavors::{
    CompatibilityToolFlavor, Flavor, SteamClientCompatToolInfo, SteamCompatibilityTool,
};
use crate::wine_cask::install::{Install, QueueCompatibilityTool, QueueCompatibilityToolState};
use crate::wine_cask::uninstall::Uninstall;
use crate::PeerMap;
use log::{debug, error, info, warn};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_tungstenite::tungstenite::Message;

pub struct WineCask {
    pub steam_util: SteamUtil,
    pub app_state: Arc<Mutex<AppState>>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct AppState {
    pub available_flavors: Vec<Flavor>,
    pub installed_compatibility_tools: Vec<SteamCompatibilityTool>,
    pub in_progress: Option<QueueCompatibilityTool>,
    pub task_queue: VecDeque<Task>,
    pub updater_state: UpdaterState,
    pub updater_last_check: Option<u64>,
    #[serde(skip)]
    pub available_compat_tools: Option<Vec<SteamClientCompatToolInfo>>,
    #[serde(skip)]
    pub flavors: Vec<Flavor>,
}

#[derive(Serialize, Deserialize, Clone)]
pub enum UpdaterState {
    Idle,
    Checking,
}

#[derive(Serialize, Deserialize, PartialEq, Clone, Debug)]
pub enum RequestType {
    RequestState,
    UpdateState,
    Notification,
    Task,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Task {
    pub r#type: TaskType,
    pub install: Option<Install>,
    pub uninstall: Option<Uninstall>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub enum TaskType {
    CheckForFlavorUpdates,
    InstallCompatibilityTool,
    CancelCompatibilityToolInstall,
    UninstallCompatibilityTool,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Request {
    pub r#type: RequestType,
    pub task: Option<Task>,
    pub notification: Option<String>,
    pub available_compat_tools: Option<Vec<SteamClientCompatToolInfo>>,
    pub app_state: Option<AppState>,
}

// Internal only
#[derive(Serialize, Deserialize, Clone)]
pub struct VirtualCompatibilityToolMetadata {
    r#virtual: bool,
    virtual_original: String,
}

impl WineCask {
    pub(crate) async fn task_queue_pop_front(&self) -> Option<Task> {
        self.app_state.lock().await.task_queue.pop_front()
    }

    pub async fn add_to_task_queue(&self, task: Task, peer_map: &PeerMap) {
        self.app_state.lock().await.task_queue.push_back(task);
        self.broadcast_app_state(peer_map).await;
    }

    pub async fn remove_or_cancel_from_task_queue(&self, task: Task, peer_map: &PeerMap) {
        let mut app_state = self.app_state.lock().await;
        if let Some(position) = app_state.task_queue.iter().position(|x| {
            x.install.clone().unwrap().release.url == task.install.clone().unwrap().release.url
        }) {
            app_state.task_queue.remove(position);
            drop(app_state);
            self.broadcast_app_state(peer_map).await;
            self.broadcast_notification(
                peer_map,
                "Cancelled: Compatibility tool installation removed from queue",
            )
            .await;
        } else if let Some(in_progress) = &mut app_state.in_progress {
            in_progress.state = QueueCompatibilityToolState::Cancelling;
            self.broadcast_notification(
                peer_map,
                "Cancelling: Compatibility tool installation in progress",
            )
            .await;
        } else {
            self.broadcast_notification(
                peer_map,
                "Not Found: Compatibility tool not found in queue",
            )
            .await;
        }
    }

    pub async fn broadcast_app_state(&self, peer_map: &PeerMap) {
        let app_state = self.app_state.lock().await;
        let response_new: Request = Request {
            r#type: RequestType::UpdateState,
            task: None,
            notification: None,
            available_compat_tools: None,
            app_state: Some(app_state.clone()),
        };
        drop(app_state);
        self.broadcast_message(peer_map, &response_new).await;
    }

    pub async fn broadcast_notification(&self, peer_map: &PeerMap, message: &str) {
        let response_new: Request = Request {
            r#type: RequestType::Notification,
            task: None,
            notification: Some(message.to_string()),
            available_compat_tools: None,
            app_state: None,
        };
        self.broadcast_message(peer_map, &response_new).await;
    }

    async fn broadcast_message(&self, peer_map: &PeerMap, response: &Request) {
        let update = serde_json::to_string(response).unwrap();
        let message = Message::text(&update);
        for recp in peer_map.lock().await.values() {
            match recp.unbounded_send(message.clone()) {
                Ok(_) => {
                    info!("Type: {:?}", response.r#type);
                    debug!("Websocket message sent: {}", &update);
                }
                Err(e) => {
                    error!("Failed to send websocket message: {}", e);
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
        let used_by_games: Vec<String> = installed_games
            .iter()
            .filter(|game| {
                compat_tools_mapping.contains_key(&game.app_id)
                    && (compat_tools_mapping
                        .get(&game.app_id)
                        .unwrap()
                        .eq(display_name)
                        || compat_tools_mapping
                            .get(&game.app_id)
                            .unwrap()
                            .eq(internal_name))
            })
            .map(|game| game.name.clone())
            .collect();
        used_by_games
    }

    pub async fn update_used_by_games(&self, peer_map: &PeerMap) {
        for compat_tool in &mut self.app_state.lock().await.installed_compatibility_tools {
            compat_tool.used_by_games =
                self.get_used_by_games(&compat_tool.display_name, &compat_tool.internal_name);
        }
        self.broadcast_app_state(peer_map).await;
    }

    pub fn list_compatibility_tools(&self) -> Option<Vec<SteamCompatibilityTool>> {
        let compat_tools = self
            .steam_util
            .list_compatibility_tools()
            .expect("Failed to get list of compatibility tools");

        let mut compatibility_tools: Vec<SteamCompatibilityTool> = Vec::new();

        for compat_tool in &compat_tools {
            let used_by_games: Vec<String> =
                self.get_used_by_games(&compat_tool.display_name, &compat_tool.internal_name);
            //let metadata = self.lookup_virtual_compatibility_tool_metadata(compat_tool);
            compatibility_tools.push(SteamCompatibilityTool {
                path: compat_tool.path.to_string_lossy().to_string(),
                //directory_name: compat_tool.directory_name.to_string(),
                display_name: compat_tool.display_name.to_string(),
                internal_name: compat_tool.internal_name.to_string(),
                used_by_games,
                flavor: CompatibilityToolFlavor::Unknown,
                github_release: None,
                requires_restart: false,
                //r#virtual: metadata.r#virtual,
                //virtual_original: metadata.virtual_original,
            })
        }

        Some(compatibility_tools)
    }

    pub async fn process_frontend_compat_tools_update(
        &self,
        peer_map: &PeerMap,
        available_compat_tools: Vec<SteamClientCompatToolInfo>,
    ) {
        let mut app_state = self.app_state.lock().await;
        app_state.available_compat_tools = Some(available_compat_tools);
        drop(app_state);
        self.sync_backend_with_installed_compat_tools().await;
        self.broadcast_app_state(peer_map).await;
    }

    pub async fn sync_backend_with_installed_compat_tools(&self) {
        let mut app_state = self.app_state.lock().await;
        app_state.installed_compatibility_tools = self.list_compatibility_tools().unwrap();

        let available_compat_tools = app_state.available_compat_tools.clone().unwrap();

        let available_tools_map: HashMap<String, &SteamClientCompatToolInfo> =
            available_compat_tools
                .iter()
                .map(|tool| (tool.str_tool_name.clone(), tool))
                .collect();

        for tool in &mut app_state.installed_compatibility_tools {
            tool.requires_restart = !available_tools_map.contains_key(&tool.internal_name);
        }
        drop(app_state);
        self.update_compatibility_tools_and_available_flavors()
            .await;
    }

    pub async fn check_for_flavor_updates(&self, peer_map: &PeerMap, renew_cache: bool) {
        self.app_state.lock().await.updater_state = UpdaterState::Checking;
        self.broadcast_app_state(peer_map).await;
        self.app_state.lock().await.flavors = self.get_flavors(renew_cache).await;
        self.app_state.lock().await.updater_state = UpdaterState::Idle;
        self.broadcast_app_state(peer_map).await;
    }
}

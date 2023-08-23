use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use log::{debug, error, info};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tokio_tungstenite::tungstenite::Message;
use crate::PeerMap;
use crate::steam_util::{CompatibilityTool, SteamUtil};
use crate::wine_cask::flavors::{Flavor, SteamClientCompatToolInfo, SteamCompatibilityTool};
use crate::wine_cask::install::{Install, QueueCompatibilityTool};
use crate::wine_cask::uninstall::Uninstall;

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
    #[serde(skip)]
    pub available_compat_tools: Option<Vec<SteamClientCompatToolInfo>>,
}

#[derive(Serialize, Deserialize, PartialEq, Clone)]
pub enum RequestType {
    RequestState,
    UpdateState,
    Install,
    Uninstall,
    Notification,
    Task,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Task {
    pub r#type: TaskType,
    pub install: Option<Install>
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub enum TaskType {
    InstallCompatibilityTool,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Request {
    pub r#type: RequestType,
    pub available_compat_tools: Option<Vec<SteamClientCompatToolInfo>>,
    pub app_state: Option<AppState>,
    pub install: Option<Install>,
    pub uninstall: Option<Uninstall>,
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

    pub async fn remove_from_task_queue(&self, install: Install) {
        let mut app_state = self.app_state.lock().await;
        if let Some(position) = app_state
            .task_queue
            .iter()
            .position(|x| x.install.clone().unwrap().release.url == install.release.url)
        {
            app_state.task_queue.remove(position);
            //Todo: Notify removed from queue
        } else {
            //Todo: Notify none found
        }
    }

    pub async fn broadcast_app_state(&self, peer_map: &PeerMap) {
        let app_state = self.app_state.lock().await;
        let response_new: Request = Request {
            r#type: RequestType::UpdateState,
            available_compat_tools: None,
            app_state: Some(app_state.clone()),
            install: None,
            uninstall: None,
        };
        let update = serde_json::to_string(&response_new).unwrap();
        let message = Message::text(&update);
        for recp in peer_map.lock().await.values() {
            match recp.unbounded_send(message.clone()) {
                Ok(_) => {
                    debug!("Websocket message sent: {}", &update);
                }
                Err(e) => {
                    error!("Failed to send websocket message: {}", e);
                }
            }
        }
    }


    // todo: We need a hook in the frontend when user sets a compatibility tool to call this function
    // our current workaround is just update it every time we get a request for the app state; which should be fine for the most part.
    // it is a problem if we choose to just use a single websocket client for handling the frontend + notifications
    fn get_used_by_games(&self, display_name: &str, internal_name: &str) -> Vec<String> {
        let compat_tools_mapping = self
            .steam_util
            .get_compatibility_tools_mappings()
            .expect("Failed to get compatibility tools mappings");
        let installed_games = self
            .steam_util
            .list_installed_games()
            .expect("Failed to get list of installed games");
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
                github_release: None,
                requires_restart: false,
                //r#virtual: metadata.r#virtual,
                //virtual_original: metadata.virtual_original,
            })
        }

        Some(compatibility_tools)
    }

    pub fn find_unlisted_directories(&self, installed_compatibility_tools: &Vec<SteamCompatibilityTool>) -> Vec<CompatibilityTool> {
        self.steam_util
            .list_compatibility_tools()
            .unwrap()
            .iter()
            .filter(|refresh| {
                !installed_compatibility_tools
                    .iter()
                    .any(|ct| refresh.internal_name == ct.internal_name)
            })
            .cloned()
            .collect()
    }

    pub fn to_steam_compatibility_tool(&self, compatibility_tool: &CompatibilityTool, requires_restart: bool) -> SteamCompatibilityTool {
        SteamCompatibilityTool {
            path: compatibility_tool.path.clone().into_os_string().into_string().unwrap(),
            display_name: compatibility_tool.display_name.to_string(),
            internal_name: compatibility_tool.internal_name.to_string(),
            used_by_games: self.get_used_by_games(&compatibility_tool.display_name, &compatibility_tool.internal_name),
            github_release: None,
            requires_restart,
        }
    }

    pub async fn process_frontend_compat_tools_update(&self, peer_map: &PeerMap, available_compat_tools: Vec<SteamClientCompatToolInfo>) {
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

        // todo: error handle this
        let available_tools_map: HashMap<String, &SteamClientCompatToolInfo> = available_compat_tools.iter().map(|tool| (tool.str_tool_name.clone(), tool)).collect();

        for tool in &mut app_state.installed_compatibility_tools {
            tool.requires_restart = !available_tools_map.contains_key(&tool.internal_name);
        }
    }
}
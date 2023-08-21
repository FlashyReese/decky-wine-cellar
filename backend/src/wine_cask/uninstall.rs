use std::path::PathBuf;
use log::error;
use serde::{Deserialize, Serialize};
use crate::PeerMap;
use crate::wine_cask::flavors::{CompatibilityToolFlavor, SteamCompatibilityTool};
use crate::wine_cask::recursive_delete_dir_entry;
use crate::wine_cask::wine_cask::WineCask;

#[derive(Serialize, Deserialize, Clone)]
pub struct Uninstall {
    pub flavor: CompatibilityToolFlavor,
    pub steam_compatibility_tool: SteamCompatibilityTool,
}

impl WineCask {
    pub async fn uninstall_compatibility_tool(&self, steam_compatibility_tool: SteamCompatibilityTool, peer_map: &PeerMap) {
        let directory_path = PathBuf::from(&steam_compatibility_tool.path);
        if let Err(e) = recursive_delete_dir_entry(&directory_path) {
            error!("Error deleting directory: {}", e);
            // todo: send a toast notification to the client
            return;
        }

        let mut app_state = self.app_state.lock().await;
        if let Some(index) = app_state
            .installed_compatibility_tools
            .iter()
            .position(|x| {
                x.internal_name == steam_compatibility_tool.internal_name
                    && x.path == steam_compatibility_tool.path
            })
        {
            app_state.installed_compatibility_tools.remove(index);
        } else {
            error!("Error removing compatibility tool from app state... it's possibly already removed?");
            // todo: send a toast notification to the client
            return;
        }

        let installed = app_state.installed_compatibility_tools.clone();
        app_state.available_flavors = self.get_flavors(installed, false).await;
        self.broadcast_app_state(peer_map).await;
    }
}
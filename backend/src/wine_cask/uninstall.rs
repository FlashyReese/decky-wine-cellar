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
            self.broadcast_notification(peer_map, "Something went wrong with the uninstallation").await;
            return;
        }

        self.sync_backend_with_installed_compat_tools().await;
        let mut app_state = self.app_state.lock().await;
        let installed = app_state.installed_compatibility_tools.clone();
        app_state.available_flavors = self.get_flavors(installed, false).await;
        drop(app_state);
        self.broadcast_app_state(peer_map).await;
    }
}
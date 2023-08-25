use crate::wine_cask::app::WineCask;
use crate::wine_cask::flavors::{CompatibilityToolFlavor, SteamCompatibilityTool};
use crate::wine_cask::recursive_delete_dir_entry;
use crate::PeerMap;
use log::error;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone)]
pub struct Uninstall {
    pub flavor: CompatibilityToolFlavor,
    pub steam_compatibility_tool: SteamCompatibilityTool,
}

impl WineCask {
    // Asynchronously uninstall a compatibility tool
    pub async fn uninstall_compatibility_tool(
        &self,
        steam_compatibility_tool: SteamCompatibilityTool,
        peer_map: &PeerMap,
    ) {
        // Validate that the compatibility tool is installed for security reason we don't want to delete something else.
        // Find the compatibility tool to uninstall
        let matching_tools: Vec<SteamCompatibilityTool> = self
            .app_state
            .lock()
            .await
            .installed_compatibility_tools
            .iter()
            .filter(|tool| {
                tool.path == steam_compatibility_tool.path
                    && tool.internal_name == steam_compatibility_tool.internal_name
                    && tool.display_name == steam_compatibility_tool.display_name
            })
            .cloned()
            .collect();

        // Handle cases when no matching tool is found
        if matching_tools.is_empty() {
            let error_message = format!(
                "Compatibility tool not found: {}",
                steam_compatibility_tool.display_name
            );
            error!("{}", error_message);
            self.broadcast_notification(peer_map, &error_message).await;
            return;
        }

        // Handle cases when multiple matching tools are found
        if matching_tools.len() != 1 {
            let error_message = format!(
                "Invalid number of matching tools found: {}",
                matching_tools.len()
            );
            error!("{}", error_message);
            self.broadcast_notification(peer_map, &error_message).await;
            return;
        }

        // Get the tool to uninstall (only one at this point)
        let tool_to_uninstall = &matching_tools[0];

        // Uninstall the compatibility tool by deleting its directory
        let directory_path = PathBuf::from(&tool_to_uninstall.path);
        if let Err(e) = recursive_delete_dir_entry(&directory_path) {
            let error_message = format!("Error during uninstallation: {}", e);
            error!("{}", error_message);
            self.broadcast_notification(peer_map, &error_message).await;
            return;
        }

        // Update the app state to reflect the uninstalled tool and broadcast changes
        self.sync_backend_with_installed_compat_tools().await;
        self.broadcast_app_state(peer_map).await;
    }
}

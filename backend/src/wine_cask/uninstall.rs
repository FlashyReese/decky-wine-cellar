use crate::wine_cask::app::WineCask;
use crate::wine_cask::flavors::InstalledToolSource;
use crate::wine_cask::recursive_delete_dir_entry;
use crate::PeerMap;
use log::error;
use std::fs;
use std::path::PathBuf;

impl WineCask {
    pub async fn uninstall_installed_tool(&self, installed_tool_id: String, peer_map: &PeerMap) {
        let Some(installed_tool) = self.get_installed_tool(&installed_tool_id).await else {
            self.broadcast_notification(peer_map, "Installed tool not found")
                .await;
            return;
        };

        if matches!(&installed_tool.source, InstalledToolSource::Virtual) {
            let Some(virtual_tool_id) = installed_tool.virtual_tool_id else {
                self.broadcast_notification(
                    peer_map,
                    "Cannot safely remove this virtual tool because its manifest identity is missing",
                )
                .await;
                return;
            };
            self.remove_virtual_tool(virtual_tool_id, peer_map).await;
            return;
        }

        let linked_virtual_tools = match self
            .virtual_tools_linking_source(&installed_tool.id, &installed_tool.directory_name)
        {
            Ok(tools) => tools,
            Err(err) => {
                let error_message = format!(
                    "Cannot safely remove this tool because virtual tool dependencies could not be verified: {}",
                    err
                );
                error!("{}", error_message);
                self.broadcast_notification(peer_map, &error_message).await;
                return;
            }
        };
        if !linked_virtual_tools.is_empty() {
            self.broadcast_notification(
                peer_map,
                &format!(
                    "Cannot remove {}; it is linked by: {}",
                    installed_tool.display_name,
                    linked_virtual_tools.join(", ")
                ),
            )
            .await;
            return;
        }

        if !matches!(&installed_tool.source, InstalledToolSource::Direct) {
            // Keep future source variants fail-closed instead of treating them as direct installs.
            self.broadcast_notification(
                peer_map,
                "Cannot safely remove an unsupported compatibility tool source",
            )
            .await;
            return;
        }

        let directory_path = PathBuf::from(&installed_tool.path);
        let base_dir = self.steam_util.get_steam_compatibility_tools_directory();

        let canonical_base = match base_dir.canonicalize() {
            Ok(base) => base,
            Err(err) => {
                let error_message = format!(
                    "Failed to access compatibility tools base directory: {}",
                    err
                );
                error!("{}", error_message);
                self.broadcast_notification(peer_map, &error_message).await;
                return;
            }
        };

        let canonical_parent = match directory_path.parent().map(PathBuf::from) {
            Some(parent) => match parent.canonicalize() {
                Ok(parent) => parent,
                Err(err) => {
                    let error_message = format!("Failed to access uninstall parent path: {}", err);
                    error!("{}", error_message);
                    self.broadcast_notification(peer_map, &error_message).await;
                    return;
                }
            },
            None => {
                self.broadcast_notification(peer_map, "Failed to resolve uninstall parent path")
                    .await;
                return;
            }
        };
        if canonical_parent != canonical_base {
            let error_message =
                "Refusing to uninstall path outside compatibilitytools.d".to_string();
            error!("{}", error_message);
            self.broadcast_notification(peer_map, &error_message).await;
            return;
        }

        let target_metadata = match fs::symlink_metadata(&directory_path) {
            Ok(metadata) => metadata,
            Err(err) => {
                let error_message = format!("Failed to access uninstall path: {}", err);
                error!("{}", error_message);
                self.broadcast_notification(peer_map, &error_message).await;
                return;
            }
        };

        if target_metadata.file_type().is_symlink() {
            if let Err(err) = fs::remove_file(&directory_path) {
                let error_message = format!("Error removing compatibility tool symlink: {}", err);
                error!("{}", error_message);
                self.broadcast_notification(peer_map, &error_message).await;
                return;
            }
        } else {
            let canonical_target = match directory_path.canonicalize() {
                Ok(path) => path,
                Err(err) => {
                    let error_message = format!("Failed to access uninstall path: {}", err);
                    error!("{}", error_message);
                    self.broadcast_notification(peer_map, &error_message).await;
                    return;
                }
            };

            if canonical_target.parent() != Some(canonical_base.as_path()) {
                let error_message =
                    "Refusing to uninstall path outside compatibilitytools.d".to_string();
                error!("{}", error_message);
                self.broadcast_notification(peer_map, &error_message).await;
                return;
            }

            if let Err(err) = recursive_delete_dir_entry(&canonical_target) {
                let error_message = format!("Error during uninstallation: {}", err);
                error!("{}", error_message);
                self.broadcast_notification(peer_map, &error_message).await;
                return;
            }
        }

        self.sync_backend_state().await;
        self.broadcast_app_state(peer_map).await;

        let label = installed_tool
            .user_label
            .unwrap_or(installed_tool.display_name);
        self.broadcast_notification(peer_map, &format!("Removed {}", label))
            .await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::steam_util::SteamUtil;
    use crate::wine_cask::app::{AppState, OperationStateSnapshot, UpdaterState};
    use crate::wine_cask::flavors::{
        CompatibilityToolFlavor, InstalledCompatibilityTool, InstalledToolSource,
    };
    use crate::wine_cask::virtual_tools::{VirtualToolConfig, VirtualToolManifest};
    use crate::PeerMap;
    use std::collections::{HashMap, VecDeque};
    use std::sync::Arc;
    use tempfile::tempdir;
    use tokio::sync::{Mutex, Notify};

    #[tokio::test]
    async fn virtual_uninstall_uses_slot_cleanup_for_reserved_directories() {
        let temp = tempdir().unwrap();
        let compatibility_tools_directory = temp.path().join("compatibilitytools.d");
        let target_directory = compatibility_tools_directory.join("WineCellarVirtual1");
        let backup_directory =
            compatibility_tools_directory.join(".wine-cellar-backup-WineCellarVirtual1");
        let staging_directory =
            compatibility_tools_directory.join(".wine-cellar-link-staging-WineCellarVirtual1");
        fs::create_dir_all(&target_directory).unwrap();
        fs::create_dir(&backup_directory).unwrap();
        fs::create_dir(&staging_directory).unwrap();

        let manifest_path = temp.path().join("runtime/virtual-tools.json");
        let installed_tool = InstalledCompatibilityTool {
            id: "installed:WineCellarVirtual1".to_string(),
            path: target_directory.to_string_lossy().to_string(),
            directory_name: "WineCellarVirtual1".to_string(),
            display_name: "Stable Proton".to_string(),
            internal_name: "WineCellarVirtual1".to_string(),
            used_by_games: Vec::new(),
            requires_restart: false,
            flavor: CompatibilityToolFlavor::ProtonGE,
            catalog_release_id: None,
            github_release: None,
            source: InstalledToolSource::Virtual,
            virtual_tool_id: Some("virtual-1".to_string()),
            user_label: Some("Stable Proton".to_string()),
            can_link_to_virtual_tool: false,
        };
        let app = WineCask {
            steam_util: SteamUtil::new(temp.path().to_path_buf()),
            app_state: Arc::new(Mutex::new(AppState {
                catalog_flavors: Vec::new(),
                installed_tools: vec![installed_tool],
                virtual_tools: Vec::new(),
                app_compat_tool_mappings: HashMap::new(),
                app_compat_tool_mappings_stale: true,
                current_operation: None,
                queued_operations: Vec::new(),
                updater_state: UpdaterState::Idle,
                updater_last_check: None,
                steam_visible_tools: Vec::new(),
                operation_queue: VecDeque::new(),
                app_compat_tool_mappings_last_refresh_attempt: None,
                app_compat_tool_mappings_refresh_in_progress: false,
            })),
            operation_broadcast_cache: Arc::new(Mutex::new(
                None::<(OperationStateSnapshot, std::time::Instant)>,
            )),
            queue_notify: Arc::new(Notify::new()),
            virtual_tool_manifest_path: manifest_path,
        };
        app.save_virtual_tool_manifest(&VirtualToolManifest {
            next_virtual_tool_number: 2,
            tools: vec![VirtualToolConfig {
                id: "virtual-1".to_string(),
                user_label: "Stable Proton".to_string(),
                steam_internal_name: "WineCellarVirtual1".to_string(),
                directory_name: "WineCellarVirtual1".to_string(),
                current_payload_release_id: None,
                current_payload_name: Some("GE-Proton".to_string()),
                current_payload_flavor: Some(CompatibilityToolFlavor::ProtonGE),
                linked_source_installed_tool_id: None,
                linked_source_directory_name: None,
                pending_payload_transaction: None,
            }],
        })
        .unwrap();

        let peer_map: PeerMap = Arc::new(Mutex::new(HashMap::new()));
        app.uninstall_installed_tool("installed:WineCellarVirtual1".to_string(), &peer_map)
            .await;

        assert!(!target_directory.exists());
        assert!(!backup_directory.exists());
        assert!(!staging_directory.exists());
        assert!(app
            .try_load_virtual_tool_manifest()
            .unwrap()
            .tools
            .is_empty());
    }
}

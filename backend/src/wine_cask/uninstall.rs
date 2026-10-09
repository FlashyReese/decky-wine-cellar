use crate::i18n::message;
use crate::wine_cask::app::WineCask;
use crate::wine_cask::recursive_delete_dir_entry;
use crate::PeerMap;
use log::error;
use std::fs;
use std::path::PathBuf;

impl WineCask {
    pub async fn uninstall_installed_tool(&self, installed_tool_id: String, peer_map: &PeerMap) {
        let Some(installed_tool) = self.get_installed_tool(&installed_tool_id).await else {
            self.broadcast_notification(peer_map, message("uninstall-toolNotFound", &[]))
                .await;
            return;
        };

        let directory_path = PathBuf::from(&installed_tool.path);
        let base_dir = self.steam_util.get_steam_compatibility_tools_directory();

        let canonical_base = match base_dir.canonicalize() {
            Ok(base) => base,
            Err(err) => {
                let error_message =
                    message("uninstall-baseFailed", &[("error", &err.to_string())]);
                error!("{}", error_message);
                self.broadcast_notification(peer_map, error_message).await;
                return;
            }
        };

        let canonical_parent = match directory_path.parent().map(PathBuf::from) {
            Some(parent) => match parent.canonicalize() {
                Ok(parent) => parent,
                Err(err) => {
                    let error_message =
                        message("uninstall-parentFailed", &[("error", &err.to_string())]);
                    error!("{}", error_message);
                    self.broadcast_notification(peer_map, error_message).await;
                    return;
                }
            },
            None => {
                self.broadcast_notification(peer_map, message("uninstall-parentNotFound", &[]))
                    .await;
                return;
            }
        };
        if canonical_parent != canonical_base {
            let error_message = message("uninstall-outsideDirectory", &[]);
            error!("{}", error_message);
            self.broadcast_notification(peer_map, error_message).await;
            return;
        }

        let target_metadata = match fs::symlink_metadata(&directory_path) {
            Ok(metadata) => metadata,
            Err(err) => {
                let error_message = message("uninstall-pathFailed", &[("error", &err.to_string())]);
                error!("{}", error_message);
                self.broadcast_notification(peer_map, error_message).await;
                return;
            }
        };

        if target_metadata.file_type().is_symlink() {
            if let Err(err) = fs::remove_file(&directory_path) {
                let error_message =
                    message("uninstall-symlinkFailed", &[("error", &err.to_string())]);
                error!("{}", error_message);
                self.broadcast_notification(peer_map, error_message).await;
                return;
            }
        } else {
            let canonical_target = match directory_path.canonicalize() {
                Ok(path) => path,
                Err(err) => {
                    let error_message =
                        message("uninstall-pathFailed", &[("error", &err.to_string())]);
                    error!("{}", error_message);
                    self.broadcast_notification(peer_map, error_message).await;
                    return;
                }
            };

            if canonical_target.parent() != Some(canonical_base.as_path()) {
                let error_message = message("uninstall-outsideDirectory", &[]);
                error!("{}", error_message);
                self.broadcast_notification(peer_map, error_message).await;
                return;
            }

            if let Err(err) = recursive_delete_dir_entry(&canonical_target) {
                let error_message = message("uninstall-failed", &[("error", &err.to_string())]);
                error!("{}", error_message);
                self.broadcast_notification(peer_map, error_message).await;
                return;
            }
        }

        self.sync_backend_state().await;
        self.broadcast_app_state(peer_map).await;

        let label = installed_tool.display_name;
        self.broadcast_notification(peer_map, message("uninstall-completed", &[("tool", &label)]))
            .await;
    }
}

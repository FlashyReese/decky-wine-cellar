use crate::wine_cask::app::{Command, OperationState, WineCask};
use crate::PeerMap;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::sync::Arc;
use std::{fs, io};

pub mod app;
pub mod flavors;
pub mod install;
pub mod uninstall;
pub mod virtual_tools;

pub fn generate_compatibility_tool_vdf<P: AsRef<Path>>(
    path: P,
    internal_name: &str,
    display_name: &str,
) -> io::Result<()> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let internal_name = escape_vdf_string(internal_name);
    let display_name = escape_vdf_string(display_name);
    let mut file = File::create(path)?;
    writeln!(
        file,
        r#""compatibilitytools"
            {{
              "compat_tools"
              {{
                "{}"
                {{
                  "install_path" "."
                  "display_name" "{}"
                  "from_oslist"  "windows"
                  "to_oslist"    "linux"
                }}
              }}
            }}"#,
        internal_name, display_name
    )
}

fn escape_vdf_string(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());

    for character in value.chars() {
        match character {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            character if character.is_control() => escaped.push(' '),
            character => escaped.push(character),
        }
    }

    escaped
}

fn recursive_delete_dir_entry(entry_path: &Path) -> io::Result<()> {
    let metadata = fs::symlink_metadata(entry_path)?;

    if metadata.file_type().is_symlink() {
        fs::remove_file(entry_path)?;
    } else if metadata.is_dir() {
        for entry in fs::read_dir(entry_path)? {
            let entry = entry?;
            let path = entry.path();
            recursive_delete_dir_entry(&path)?;
        }
        fs::remove_dir(entry_path)?;
    } else {
        fs::remove_file(entry_path)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn generated_vdf_escapes_strings() {
        let directory = tempdir().expect("Failed to create temp directory");
        let vdf_path = directory.path().join("compatibilitytool.vdf");

        generate_compatibility_tool_vdf(&vdf_path, r#"Wine"Cellar\Virtual"#, "Display\nName")
            .expect("Failed to generate VDF");

        let contents = fs::read_to_string(vdf_path).expect("Failed to read VDF");
        assert!(contents.contains(r#""Wine\"Cellar\\Virtual""#));
        assert!(contents.contains(r#""display_name" "Display\nName""#));
    }
}

pub async fn process_queue(wine_cask: Arc<WineCask>, peer_map: PeerMap) {
    wine_cask.check_for_flavor_updates(&peer_map, false).await;

    loop {
        if let Some(queued_operation) = wine_cask.begin_next_operation(&peer_map).await {
            match queued_operation.command {
                Command::InstallCatalogRelease { release_id, target } => {
                    wine_cask
                        .install_catalog_release(release_id, target, &peer_map)
                        .await;
                }
                Command::UninstallInstalledTool { installed_tool_id } => {
                    wine_cask
                        .update_current_operation(OperationState::Running, 0, &peer_map)
                        .await;
                    wine_cask
                        .uninstall_installed_tool(installed_tool_id, &peer_map)
                        .await;
                }
                Command::CreateVirtualTool { user_label } => {
                    wine_cask
                        .update_current_operation(OperationState::Running, 0, &peer_map)
                        .await;
                    match wine_cask.create_virtual_tool_slot(user_label) {
                        Ok(label) => {
                            wine_cask.sync_backend_state().await;
                            wine_cask.broadcast_app_state(&peer_map).await;
                            wine_cask
                                .broadcast_notification(
                                    &peer_map,
                                    &format!("Created virtual compatibility tool: {}", label),
                                )
                                .await;
                        }
                        Err(err) => {
                            wine_cask.broadcast_notification(&peer_map, &err).await;
                        }
                    }
                }
                Command::RenameVirtualTool {
                    virtual_tool_id,
                    user_label,
                } => {
                    wine_cask
                        .update_current_operation(OperationState::Running, 0, &peer_map)
                        .await;
                    match wine_cask.rename_virtual_tool_slot(&virtual_tool_id, user_label) {
                        Ok(label) => {
                            wine_cask.sync_backend_state().await;
                            wine_cask.broadcast_app_state(&peer_map).await;
                            wine_cask
                                .broadcast_notification(
                                    &peer_map,
                                    &format!("Renamed virtual compatibility tool to {}", label),
                                )
                                .await;
                        }
                        Err(err) => {
                            wine_cask.broadcast_notification(&peer_map, &err).await;
                        }
                    }
                }
                Command::RemoveVirtualTool { virtual_tool_id } => {
                    wine_cask
                        .update_current_operation(OperationState::Running, 0, &peer_map)
                        .await;
                    wine_cask
                        .remove_virtual_tool(virtual_tool_id, &peer_map)
                        .await;
                }
                Command::RefreshCatalog | Command::CancelOperation { .. } => {}
            }

            wine_cask.complete_current_operation(&peer_map).await;
            wine_cask.reclaim_memory_if_idle(&peer_map).await;
            continue;
        }

        wine_cask.queue_notify.notified().await;
    }
}

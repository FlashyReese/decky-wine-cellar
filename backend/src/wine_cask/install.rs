use crate::github_util::{Asset, Release};
use crate::wine_cask::app::WineCask;
use crate::wine_cask::flavors::CompatibilityToolFlavor;
use crate::wine_cask::{copy_dir, generate_compatibility_tool_vdf, recursive_delete_dir_entry};
use crate::PeerMap;
use flate2::bufread::GzDecoder;
use futures_util::StreamExt;
use log::{debug, error, info, warn};
use serde::{Deserialize, Serialize};
use std::env;
use std::fs::create_dir_all;
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};
use xz2::bufread::XzDecoder;

#[derive(Serialize, Deserialize, Clone)]
pub struct Install {
    pub(crate) flavor: CompatibilityToolFlavor,
    pub(crate) release: Release,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct QueueCompatibilityTool {
    pub flavor: CompatibilityToolFlavor,
    pub name: String,
    pub url: String,
    pub state: QueueCompatibilityToolState,
    pub compress_type: CompressionType,
    pub progress: u8,
}

#[derive(Deserialize, Serialize, PartialEq, Clone)]
pub enum QueueCompatibilityToolState {
    Extracting,
    Downloading,
    Waiting,
    Cancelling,
}

#[derive(Deserialize, Serialize, PartialEq, Clone)]
pub enum CompressionType {
    Gzip,
    Xz,
    Unknown,
}

impl WineCask {
    // Why is this task queue here? Well because steam deck will die if someone tries to queue up 50 installs at once.
    pub async fn install_compatibility_tool(&self, install: Install, peer_map: &PeerMap) {
        if let Some(mut queue_compatibility_tool) = look_for_compressed_archive(&install) {
            // Mark as downloading...
            queue_compatibility_tool.state = QueueCompatibilityToolState::Downloading;
            queue_compatibility_tool.progress = 0;
            self.app_state.lock().await.in_progress = Some(queue_compatibility_tool.clone());
            self.broadcast_app_state(peer_map).await;

            // Starting download compatibility tool
            let client = reqwest::Client::new();
            let response_wrapped = client.get(&queue_compatibility_tool.url).send().await;
            let response = response_wrapped.unwrap();
            let total_size = response.content_length().unwrap_or(0);

            let mut downloaded_bytes = Vec::new();
            let mut downloaded_size = 0;
            let mut body = response.bytes_stream();

            while let Some(chunk_result) = body.next().await {
                // Check if we need to cancel the download
                if self
                    .app_state
                    .lock()
                    .await
                    .in_progress
                    .clone()
                    .unwrap()
                    .state
                    == QueueCompatibilityToolState::Cancelling
                {
                    self.app_state.lock().await.in_progress = None;
                    self.broadcast_app_state(peer_map).await;
                    return; // We stop the function here
                }
                if let Ok(chunk) = chunk_result {
                    downloaded_bytes.extend_from_slice(&chunk);
                    downloaded_size += chunk.len() as u64;

                    let progress = ((downloaded_size as f64 / total_size as f64) * 100.0) as u8;
                    if queue_compatibility_tool.progress != progress {
                        // Update progress...
                        queue_compatibility_tool.progress = progress;
                        self.app_state.lock().await.in_progress =
                            Some(queue_compatibility_tool.clone());
                        self.broadcast_app_state(peer_map).await;
                    }
                } else {
                    let error_message =
                        "Connection Error: Download in progress failed!".to_string();
                    error!("{}", error_message);
                    self.app_state.lock().await.in_progress = None;
                    self.broadcast_app_state(peer_map).await;
                    self.broadcast_notification(peer_map, error_message.as_str())
                        .await;
                    return;
                }
            }

            let reader = Cursor::new(downloaded_bytes);

            self.extract_generate_and_move(
                peer_map,
                &install,
                &mut queue_compatibility_tool,
                reader,
            )
            .await;
        } else {
        }
    }

    pub async fn extract_generate_and_move(
        &self,
        peer_map: &PeerMap,
        install: &Install,
        queue_compatibility_tool: &mut QueueCompatibilityTool,
        reader: Cursor<Vec<u8>>,
    ) {
        if let Some(temp_dir) = prepare_temp_directory() {
            // Mark as extracting...
            queue_compatibility_tool.state = QueueCompatibilityToolState::Extracting;
            queue_compatibility_tool.progress = 0;
            self.app_state.lock().await.in_progress = Some(queue_compatibility_tool.clone());
            self.broadcast_app_state(peer_map).await;

            let steam_compatibility_tools_directory =
                self.steam_util.get_steam_compatibility_tools_directory();
            // Spawn a new thread for the extraction process
            // Why do we need this turns out unpack process is blocking, because of this async function doesn't yield control back to Rust runtime until the extraction is finished.
            let queue_compatibility_tool_clone = queue_compatibility_tool.clone(); // Clone the queue_compatibility_tool
            let temp_dir_clone = temp_dir.clone();
            tokio::task::spawn_blocking(move || {
                let decompressed: Box<dyn Read> =
                    if queue_compatibility_tool_clone.compress_type == CompressionType::Gzip {
                        Box::new(GzDecoder::new(reader))
                    } else if queue_compatibility_tool_clone.compress_type == CompressionType::Xz {
                        Box::new(XzDecoder::new(reader))
                    } else {
                        Box::new(reader) // fixme: explosion
                    };
                let mut tar = tar::Archive::new(decompressed);
                tar.unpack(temp_dir_clone).unwrap();
            })
            .await
            .unwrap();

            // Scan for the extracted directory
            let valid_directories: Vec<PathBuf> = std::fs::read_dir(&temp_dir)
                .map_err(|_err| {
                    error!("Failed to read directory");
                })
                .unwrap()
                .filter_map(Result::ok)
                .filter(|x| {
                    x.metadata().unwrap().is_dir()
                        && x.path().join("compatibilitytool.vdf").exists()
                })
                .map(|x| x.path())
                .collect();

            if valid_directories.len() == 1 {
                let first = valid_directories.get(0).unwrap();
                let new_compat_tool_vdf = first.join("compatibilitytool.vdf");
                let new_path = match queue_compatibility_tool.flavor {
                    CompatibilityToolFlavor::ProtonGE => first.clone(),
                    CompatibilityToolFlavor::SteamTinkerLaunch
                    | CompatibilityToolFlavor::Luxtorpeda
                    | CompatibilityToolFlavor::Boxtron => {
                        let new_folder_name = format!(
                            "{}{}",
                            &queue_compatibility_tool.flavor, &install.release.tag_name
                        );
                        generate_compatibility_tool_vdf(
                            new_compat_tool_vdf,
                            &new_folder_name,
                            &format!(
                                "{} {}",
                                &queue_compatibility_tool.flavor, &install.release.tag_name
                            ),
                        );
                        temp_dir.join(&new_folder_name)
                    }
                    _ => {
                        error!("Unsupported compatibility tool flavor");
                        first.clone()
                    }
                };
                std::fs::rename(first, &new_path).unwrap();

                match copy_dir(&temp_dir, &steam_compatibility_tools_directory) {
                    Ok(_) => debug!("Directory copied successfully."),
                    Err(e) => error!("Failed to copy directory: {}", e),
                }

                self.sync_backend_with_installed_compat_tools().await;
                self.broadcast_app_state(peer_map).await;
            } else {
                error!("Failed to find extracted directory");
            }

            cleanup_temp_directory(&temp_dir);

            // Mark as completed
            let message = format!("Installation Completed: {}", install.release.name);
            info!("{}", message);
            self.broadcast_notification(peer_map, message.as_str())
                .await;
            self.app_state.lock().await.in_progress = None;
            self.broadcast_app_state(peer_map).await;
        } else {
            error!("Failed to prepare temp directory");
        }
    }
}

fn prepare_temp_directory() -> Option<PathBuf> {
    let temp_dir = PathBuf::from(
        env::var("DECKY_PLUGIN_RUNTIME_DIR").unwrap_or("/tmp/decky-wine-cellar".to_string()),
    )
    .join("temp");

    if temp_dir.exists() {
        warn!("Found existing temp directory, cleaning up...");
        cleanup_temp_directory(&temp_dir);
    }

    if let Err(err) = create_dir_all(&temp_dir) {
        error!("Failed to create temp directory: {}", err);
        return None;
    }

    Some(temp_dir)
}

fn cleanup_temp_directory(temp_dir: &Path) {
    if let Err(err) = recursive_delete_dir_entry(temp_dir) {
        error!("Failed to clean up temp directory: {}", err);
    }
}

pub fn look_for_compressed_archive(install_request: &Install) -> Option<QueueCompatibilityTool> {
    /*if install_request.flavor == CompatibilityToolFlavor::SteamTinkerLaunch {// fixme: doesn't actually work we need to handle this STL separately
        return Some(QueueCompatibilityTool {
            flavor: install_request.flavor.to_owned(),
            name: install_request.release.tag_name.to_owned(),
            url: format!("https://codeload.github.com/sonic2kk/steamtinkerlaunch/legacy.tar.gz/refs/tags/{}", install_request.release.tag_name), //install_request.release.tarball_url.to_owned(),
            state: QueueCompatibilityToolState::Waiting,
            compress_type: CompressionType::Gzip,
            progress: 0,
        });
    }*/

    let is_compressed = |asset: &Asset| {
        asset.content_type == "application/gzip" || asset.content_type == "application/x-xz"
    };

    let compress_type = |asset: &Asset| {
        if asset.content_type == "application/gzip" {
            CompressionType::Gzip
        } else if asset.content_type == "application/x-xz" {
            CompressionType::Xz
        } else {
            CompressionType::Unknown
        }
    };

    if let Some(asset) = install_request
        .release
        .assets
        .clone()
        .into_iter()
        .find(is_compressed)
    {
        return Some(QueueCompatibilityTool {
            flavor: install_request.flavor.to_owned(),
            name: install_request.release.tag_name.to_owned(),
            url: asset.clone().browser_download_url,
            state: QueueCompatibilityToolState::Waiting,
            compress_type: compress_type(&asset),
            progress: 0,
        });
    }

    None
}

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

#[derive(Debug)]
pub enum InstallError {
    NetworkError(String),
    DownloadFailed(String),
    ExtractionFailed(String),
    InstallationFailed(String),
    Cancelled,
    InvalidAsset,
    IoError(String),
}

impl std::fmt::Display for InstallError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InstallError::NetworkError(msg) => write!(f, "Network error: {}", msg),
            InstallError::DownloadFailed(msg) => write!(f, "Download failed: {}", msg),
            InstallError::ExtractionFailed(msg) => write!(f, "Extraction failed: {}", msg),
            InstallError::InstallationFailed(msg) => write!(f, "Installation failed: {}", msg),
            InstallError::Cancelled => write!(f, "Installation cancelled"),
            InstallError::InvalidAsset => write!(f, "Invalid or missing asset"),
            InstallError::IoError(msg) => write!(f, "I/O error: {}", msg),
        }
    }
}

impl WineCask {
    /// Installs a compatibility tool with proper error handling and notifications
    pub async fn install_compatibility_tool(&self, install: Install, peer_map: &PeerMap) {
        // Check if we have a valid compressed archive to download
        let queue_compatibility_tool = match look_for_compressed_archive(&install) {
            Some(tool) => tool,
            None => {
                let error_msg = format!(
                    "Installation Failed: No compatible archive found for {}",
                    install.release.name
                );
                error!("{}", error_msg);
                self.broadcast_notification(peer_map, &error_msg).await;
                return;
            }
        };

        let mut queue_compatibility_tool = queue_compatibility_tool;

        // Mark as downloading...
        queue_compatibility_tool.state = QueueCompatibilityToolState::Downloading;
        queue_compatibility_tool.progress = 0;
        self.app_state.lock().await.in_progress = Some(queue_compatibility_tool.clone());
        self.broadcast_app_state(peer_map).await;

        // Attempt download with error handling
        let download_result = self.download_compatibility_tool(
            &queue_compatibility_tool,
            peer_map,
        ).await;

        let (downloaded_bytes, expected_size) = match download_result {
            Ok(bytes) => bytes,
            Err(InstallError::Cancelled) => {
                // User cancelled, already cleaned up
                return;
            }
            Err(e) => {
                let error_msg = format!("Installation Failed: {}", e);
                error!("{}", error_msg);
                self.app_state.lock().await.in_progress = None;
                self.broadcast_app_state(peer_map).await;
                self.broadcast_notification(peer_map, &error_msg).await;
                return;
            }
        };

        // Validate downloaded size
        if expected_size > 0 && downloaded_bytes.len() as u64 != expected_size {
            let error_msg = format!(
                "Installation Failed: Downloaded file size mismatch (expected {} bytes, got {} bytes)",
                expected_size,
                downloaded_bytes.len()
            );
            error!("{}", error_msg);
            self.app_state.lock().await.in_progress = None;
            self.broadcast_app_state(peer_map).await;
            self.broadcast_notification(peer_map, &error_msg).await;
            return;
        }

        // Validate downloaded bytes are not empty
        if downloaded_bytes.is_empty() {
            let error_msg = "Installation Failed: Downloaded file is empty".to_string();
            error!("{}", error_msg);
            self.app_state.lock().await.in_progress = None;
            self.broadcast_app_state(peer_map).await;
            self.broadcast_notification(peer_map, &error_msg).await;
            return;
        }

        info!(
            "Successfully downloaded {} ({} bytes)",
            install.release.name,
            downloaded_bytes.len()
        );

        let reader = Cursor::new(downloaded_bytes);

        // Extract and install
        let extract_result = self.extract_generate_and_move(
            peer_map,
            &install,
            &mut queue_compatibility_tool,
            reader,
        ).await;

        // Clean up in_progress state
        self.app_state.lock().await.in_progress = None;
        self.broadcast_app_state(peer_map).await;

        // Send appropriate notification based on result
        match extract_result {
            Ok(()) => {
                let message = format!("Installation Completed: {}", install.release.name);
                info!("{}", message);
                self.broadcast_notification(peer_map, &message).await;
            }
            Err(InstallError::Cancelled) => {
                // Already handled
            }
            Err(e) => {
                let error_msg = format!("Installation Failed: {}", e);
                error!("{}", error_msg);
                self.broadcast_notification(peer_map, &error_msg).await;
            }
        }
    }

    /// Downloads compatibility tool with progress tracking and cancellation support
    async fn download_compatibility_tool(
        &self,
        queue_compatibility_tool: &QueueCompatibilityTool,
        peer_map: &PeerMap,
    ) -> Result<(Vec<u8>, u64), InstallError> {
        let client = reqwest::Client::new();

        info!(
            "Starting download from: {}",
            queue_compatibility_tool.url
        );

        let response = client
            .get(&queue_compatibility_tool.url)
            .send()
            .await
            .map_err(|e| InstallError::NetworkError(e.to_string()))?;

        // Check HTTP status
        if !response.status().is_success() {
            return Err(InstallError::DownloadFailed(format!(
                "HTTP {}",
                response.status()
            )));
        }

        let total_size = response.content_length().unwrap_or(0);

        if total_size == 0 {
            warn!("Content-Length header missing or zero, download progress may be inaccurate");
        }

        let mut downloaded_bytes = Vec::new();
        let mut downloaded_size = 0u64;
        let mut body = response.bytes_stream();

        while let Some(chunk_result) = body.next().await {
            // Check if we need to cancel the download
            let should_cancel = {
                let app_state = self.app_state.lock().await;
                if let Some(ref in_progress) = app_state.in_progress {
                    in_progress.state == QueueCompatibilityToolState::Cancelling
                } else {
                    false
                }
            };

            if should_cancel {
                info!("Download cancelled by user");
                self.app_state.lock().await.in_progress = None;
                self.broadcast_app_state(peer_map).await;
                return Err(InstallError::Cancelled);
            }

            match chunk_result {
                Ok(chunk) => {
                    downloaded_bytes.extend_from_slice(&chunk);
                    downloaded_size += chunk.len() as u64;

                    // Update progress
                    if total_size > 0 {
                        let progress = ((downloaded_size as f64 / total_size as f64) * 100.0) as u8;
                        let mut should_broadcast = false;
                        {
                            let mut app_state = self.app_state.lock().await;
                            if let Some(ref mut in_progress) = app_state.in_progress {
                                if in_progress.progress != progress {
                                    in_progress.progress = progress;
                                    should_broadcast = true;
                                }
                            }
                        }
                        if should_broadcast {
                            self.broadcast_app_state(peer_map).await;
                        }
                    }
                }
                Err(e) => {
                    return Err(InstallError::DownloadFailed(e.to_string()));
                }
            }
        }

        Ok((downloaded_bytes, total_size))
    }

    /// Extracts, generates VDF if needed, and moves to Steam directory
    pub async fn extract_generate_and_move(
        &self,
        peer_map: &PeerMap,
        install: &Install,
        queue_compatibility_tool: &mut QueueCompatibilityTool,
        reader: Cursor<Vec<u8>>,
    ) -> Result<(), InstallError> {
        let temp_dir = prepare_temp_directory()
            .ok_or_else(|| InstallError::IoError("Failed to create temp directory".to_string()))?;

        // Mark as extracting...
        queue_compatibility_tool.state = QueueCompatibilityToolState::Extracting;
        queue_compatibility_tool.progress = 0;
        self.app_state.lock().await.in_progress = Some(queue_compatibility_tool.clone());
        self.broadcast_app_state(peer_map).await;

        let steam_compatibility_tools_directory =
            self.steam_util.get_steam_compatibility_tools_directory();

        // Spawn a blocking thread for extraction
        let queue_compatibility_tool_clone = queue_compatibility_tool.clone();
        let temp_dir_clone = temp_dir.clone();

        let extraction_result = tokio::task::spawn_blocking(move || {
            let decompressed: Box<dyn Read> =
                if queue_compatibility_tool_clone.compress_type == CompressionType::Gzip {
                    Box::new(GzDecoder::new(reader))
                } else if queue_compatibility_tool_clone.compress_type == CompressionType::Xz {
                    Box::new(XzDecoder::new(reader))
                } else {
                    Box::new(reader)
                };

            let mut tar = tar::Archive::new(decompressed);
            tar.unpack(&temp_dir_clone)
                .map_err(|e| InstallError::ExtractionFailed(e.to_string()))
        })
        .await
        .map_err(|e| InstallError::ExtractionFailed(format!("Task join error: {}", e)))?;

        // Check if extraction succeeded
        extraction_result?;

        info!("Extraction completed successfully");

        // Scan for the extracted directory
        let valid_directories: Vec<PathBuf> = match std::fs::read_dir(&temp_dir) {
            Ok(entries) => entries
                .filter_map(|entry| entry.ok())
                .filter(|entry| {
                    entry.metadata().ok().map(|m| m.is_dir()).unwrap_or(false)
                        && entry.path().join("compatibilitytool.vdf").exists()
                })
                .map(|entry| entry.path())
                .collect(),
            Err(e) => {
                cleanup_temp_directory(&temp_dir);
                return Err(InstallError::IoError(format!(
                    "Failed to read extracted directory: {}",
                    e
                )));
            }
        };

        if valid_directories.len() != 1 {
            let error_msg = format!(
                "Expected 1 valid directory, found {}",
                valid_directories.len()
            );
            cleanup_temp_directory(&temp_dir);
            return Err(InstallError::InstallationFailed(error_msg));
        }

        let first = valid_directories.first().unwrap();
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
                cleanup_temp_directory(&temp_dir);
                return Err(InstallError::InstallationFailed(
                    "Unsupported compatibility tool flavor".to_string(),
                ));
            }
        };

        // Rename directory if needed
        if first != &new_path {
            if let Err(e) = std::fs::rename(first, &new_path) {
                cleanup_temp_directory(&temp_dir);
                return Err(InstallError::IoError(format!("Failed to rename directory: {}", e)));
            }
        }

        // Copy to Steam directory
        let copy_result = copy_dir(&temp_dir, &steam_compatibility_tools_directory);
        cleanup_temp_directory(&temp_dir);

        match copy_result {
            Ok(_) => {
                debug!("Directory copied successfully");
                self.sync_backend_with_installed_compat_tools().await;
                self.broadcast_app_state(peer_map).await;
                Ok(())
            }
            Err(e) => Err(InstallError::IoError(format!(
                "Failed to copy to Steam directory: {}",
                e
            ))),
        }
    }
}

fn prepare_temp_directory() -> Option<PathBuf> {
    let temp_dir = PathBuf::from(
        env::var("DECKY_PLUGIN_RUNTIME_DIR").unwrap_or_else(|_| "/tmp/decky-wine-cellar".to_string()),
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
    let is_compressed = |asset: &Asset| {
        asset.content_type == "application/gzip"
            || asset.content_type == "application/x-xz"
            || asset.name.ends_with(".tar.gz")
            || asset.name.ends_with(".tar.xz")
    };

    let compress_type = |asset: &Asset| {
        if asset.content_type == "application/gzip" || asset.name.ends_with(".tar.gz") {
            CompressionType::Gzip
        } else if asset.content_type == "application/x-xz" || asset.name.ends_with(".tar.xz") {
            CompressionType::Xz
        } else {
            CompressionType::Unknown
        }
    };

    if let Some(asset) = install_request
        .release
        .assets
        .iter()
        .find(|asset| is_compressed(asset))
    {
        return Some(QueueCompatibilityTool {
            flavor: install_request.flavor.to_owned(),
            name: install_request.release.tag_name.to_owned(),
            url: asset.browser_download_url.clone(),
            state: QueueCompatibilityToolState::Waiting,
            compress_type: compress_type(asset),
            progress: 0,
        });
    }

    None
}

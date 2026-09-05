use crate::github_util::{Asset, Release};
use crate::wine_cask::app::{InstallTarget, OperationState, WineCask};
use crate::wine_cask::download_progress::DownloadProgressTracker;
use crate::wine_cask::flavors::{CatalogRelease, CompatibilityToolFlavor};
use crate::wine_cask::{generate_compatibility_tool_vdf, recursive_delete_dir_entry};
use crate::PeerMap;
use flate2::bufread::GzDecoder;
use futures_util::StreamExt;
use log::{error, info, warn};
use std::fs::{create_dir_all, File};
use std::io::{BufReader, Read};
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, Instant};
use tokio::fs::File as TokioFile;
use tokio::io::AsyncWriteExt;
use tokio::time::{interval_at, MissedTickBehavior};
use xz2::bufread::XzDecoder;

const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(600);
const DOWNLOAD_STATUS_INTERVAL: Duration = Duration::from_millis(500);
const MAX_ARCHIVE_SIZE_BYTES: u64 = 4 * 1024 * 1024 * 1024;
const MAX_EXTRACTED_SIZE_BYTES: u64 = 8 * 1024 * 1024 * 1024;
const MAX_ARCHIVE_ENTRIES: usize = 25_000;

#[derive(Clone)]
struct InstallPlan {
    catalog_release: CatalogRelease,
    target: InstallTarget,
}

#[derive(Clone, Debug, PartialEq)]
enum CompressionType {
    Gzip,
    Xz,
    Unknown,
}

struct DownloadPlan {
    url: String,
    compression_type: CompressionType,
    expected_size: u64,
}

impl WineCask {
    pub async fn install_catalog_release(
        &self,
        release_id: String,
        target: InstallTarget,
        peer_map: &PeerMap,
    ) {
        let Some(catalog_release) = self.get_catalog_release(&release_id).await else {
            self.broadcast_notification(peer_map, "Requested release is no longer available")
                .await;
            return;
        };

        let install_plan = InstallPlan {
            catalog_release,
            target: target.clone(),
        };

        let Some(download_plan) =
            look_for_compressed_archive(&install_plan.catalog_release.release)
        else {
            self.broadcast_notification(
                peer_map,
                "Error: No supported compressed archive found for this release",
            )
            .await;
            return;
        };

        if download_plan.expected_size > MAX_ARCHIVE_SIZE_BYTES {
            self.broadcast_notification(
                peer_map,
                "Error: Compatibility tool archive is larger than the supported limit",
            )
            .await;
            return;
        }

        self.update_current_operation(OperationState::Downloading, 0, peer_map)
            .await;

        let temp_dir = match prepare_temp_directory(
            &self
                .steam_util
                .get_steam_compatibility_tools_directory()
                .join(".wine-cellar-staging"),
        ) {
            Some(temp_dir) => temp_dir,
            None => {
                self.broadcast_notification(
                    peer_map,
                    "Failed to prepare temporary install directory",
                )
                .await;
                return;
            }
        };
        let archive_path = temp_dir.join(download_archive_name(&download_plan.compression_type));
        let mut archive_file = match TokioFile::create(&archive_path).await {
            Ok(file) => file,
            Err(err) => {
                cleanup_temp_directory(&temp_dir);
                self.broadcast_notification(
                    peer_map,
                    &format!("Failed to create temporary archive file: {}", err),
                )
                .await;
                return;
            }
        };

        let client = match reqwest::Client::builder().timeout(DOWNLOAD_TIMEOUT).build() {
            Ok(client) => client,
            Err(err) => {
                error!("Failed to create download client: {}", err);
                cleanup_temp_directory(&temp_dir);
                self.broadcast_notification(
                    peer_map,
                    "Connection error: Unable to prepare compatibility tool download",
                )
                .await;
                return;
            }
        };
        let response = match client.get(&download_plan.url).send().await {
            Ok(resp) => resp,
            Err(err) => {
                error!("Download request failed: {}", err);
                cleanup_temp_directory(&temp_dir);
                self.broadcast_notification(
                    peer_map,
                    "Connection error: Unable to start compatibility tool download",
                )
                .await;
                return;
            }
        };

        if !response.status().is_success() {
            error!("Download failed with status {}", response.status());
            cleanup_temp_directory(&temp_dir);
            self.broadcast_notification(peer_map, "Connection error: Download failed")
                .await;
            return;
        }

        let total_size = response
            .content_length()
            .filter(|size| *size > 0)
            .or((download_plan.expected_size > 0).then_some(download_plan.expected_size));
        if total_size
            .map(|size| size > MAX_ARCHIVE_SIZE_BYTES)
            .unwrap_or(false)
        {
            cleanup_temp_directory(&temp_dir);
            self.broadcast_notification(
                peer_map,
                "Error: Compatibility tool archive is larger than the supported limit",
            )
            .await;
            return;
        }
        let mut downloaded_size = 0u64;
        let mut body = response.bytes_stream();
        let mut download_progress = DownloadProgressTracker::new(total_size, Instant::now());
        let mut status_timer = interval_at(
            tokio::time::Instant::now() + DOWNLOAD_STATUS_INTERVAL,
            DOWNLOAD_STATUS_INTERVAL,
        );
        status_timer.set_missed_tick_behavior(MissedTickBehavior::Skip);
        self.update_current_download(download_progress.snapshot(0, Instant::now()), peer_map)
            .await;

        loop {
            if self.current_operation_is_cancelling().await {
                cleanup_temp_directory(&temp_dir);
                self.broadcast_notification(peer_map, "Installation cancelled")
                    .await;
                return;
            }

            // Keep speed and remaining time current even when the server stops sending data.
            // Sampling on a timer also avoids broadcasting once per network chunk.
            let chunk_result = tokio::select! {
                chunk = body.next() => match chunk {
                    Some(chunk) => chunk,
                    None => break,
                },
                _ = status_timer.tick() => {
                    self.update_current_download(
                        download_progress.snapshot(downloaded_size, Instant::now()),
                        peer_map,
                    )
                    .await;
                    continue;
                }
            };

            let chunk = match chunk_result {
                Ok(chunk) => chunk,
                Err(err) => {
                    error!("Download stream failed: {}", err);
                    cleanup_temp_directory(&temp_dir);
                    self.broadcast_notification(peer_map, "Connection error: Download interrupted")
                        .await;
                    return;
                }
            };

            if let Err(err) = archive_file.write_all(&chunk).await {
                error!("Failed to write temporary archive: {}", err);
                cleanup_temp_directory(&temp_dir);
                self.broadcast_notification(
                    peer_map,
                    "Storage error: Failed to write compatibility tool archive",
                )
                .await;
                return;
            }
            downloaded_size += chunk.len() as u64;
            if downloaded_size > MAX_ARCHIVE_SIZE_BYTES {
                cleanup_temp_directory(&temp_dir);
                self.broadcast_notification(
                    peer_map,
                    "Error: Compatibility tool archive is larger than the supported limit",
                )
                .await;
                return;
            }
        }

        self.update_current_download(
            download_progress.snapshot(downloaded_size, Instant::now()),
            peer_map,
        )
        .await;

        if let Err(err) = archive_file.flush().await {
            error!("Failed to flush temporary archive: {}", err);
            cleanup_temp_directory(&temp_dir);
            self.broadcast_notification(
                peer_map,
                "Storage error: Failed to finalize compatibility tool archive",
            )
            .await;
            return;
        }

        drop(archive_file);

        match self
            .extract_and_install(
                peer_map,
                &install_plan,
                download_plan.compression_type,
                &temp_dir,
                &archive_path,
            )
            .await
        {
            Ok(message) => {
                info!("{}", message);
                self.sync_backend_state().await;
                self.broadcast_app_state(peer_map).await;
                self.broadcast_notification(peer_map, &message).await;
            }
            Err(err) => {
                error!("Installation failed: {}", err);
                self.broadcast_notification(peer_map, &err).await;
            }
        }

        cleanup_temp_directory(&temp_dir);
    }

    async fn extract_and_install(
        &self,
        peer_map: &PeerMap,
        install_plan: &InstallPlan,
        compression_type: CompressionType,
        temp_dir: &Path,
        archive_path: &Path,
    ) -> Result<String, String> {
        if self.current_operation_is_cancelling().await {
            return Err("Installation cancelled".to_string());
        }

        self.update_current_operation(OperationState::Extracting, 0, peer_map)
            .await;

        let temp_dir_clone = temp_dir.to_path_buf();
        let archive_path_clone = archive_path.to_path_buf();
        let unpack_result = tokio::task::spawn_blocking(move || {
            let archive_file = File::open(&archive_path_clone)
                .map_err(|err| format!("Failed to open temporary archive: {}", err))?;
            let archive_reader = BufReader::new(archive_file);

            let decompressed: Box<dyn Read> = if compression_type == CompressionType::Gzip {
                Box::new(GzDecoder::new(archive_reader))
            } else if compression_type == CompressionType::Xz {
                Box::new(XzDecoder::new(archive_reader))
            } else {
                return Err("Unsupported archive compression type".to_string());
            };

            safe_unpack_tar(decompressed, &temp_dir_clone)
        })
        .await
        .map_err(|err| format!("Extraction task failed: {}", err))?;

        if let Err(err) = unpack_result {
            return Err(format!("Installation failed: {}", err));
        }

        if let Err(err) = std::fs::remove_file(archive_path) {
            warn!(
                "Failed to delete temporary archive after extraction: {}",
                err
            );
        }

        if self.current_operation_is_cancelling().await {
            return Err("Installation cancelled".to_string());
        }

        let extracted_directory = std::fs::read_dir(temp_dir)
            .map_err(|err| format!("Failed to read extraction directory: {}", err))?
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .file_type()
                    .map(|file_type| file_type.is_dir())
                    .unwrap_or(false)
            })
            .map(|entry| entry.path())
            .find(|path| path.join("compatibilitytool.vdf").exists())
            .ok_or_else(|| {
                "Failed to find the extracted compatibility tool contents".to_string()
            })?;

        validate_extracted_symlinks(&extracted_directory)
            .map_err(|err| format!("Installation failed: {}", err))?;

        if self.current_operation_is_cancelling().await {
            return Err("Installation cancelled".to_string());
        }

        match &install_plan.target {
            InstallTarget::Direct => self.install_direct_tool(&extracted_directory, install_plan),
            InstallTarget::VirtualTool { virtual_tool_id } => {
                self.install_virtual_tool(&extracted_directory, install_plan, virtual_tool_id)
            }
        }
    }

    fn install_direct_tool(
        &self,
        extracted_directory: &Path,
        install_plan: &InstallPlan,
    ) -> Result<String, String> {
        let temp_dir = extracted_directory
            .parent()
            .ok_or_else(|| "Missing temporary extraction parent directory".to_string())?;
        let compatibility_tools_directory =
            self.steam_util.get_steam_compatibility_tools_directory();

        let new_path = match install_plan.catalog_release.flavor {
            CompatibilityToolFlavor::ProtonGE | CompatibilityToolFlavor::ProtonCachyOS => {
                extracted_directory.to_path_buf()
            }
            CompatibilityToolFlavor::SteamTinkerLaunch
            | CompatibilityToolFlavor::Luxtorpeda
            | CompatibilityToolFlavor::Boxtron => {
                let new_folder_name = format!(
                    "{}{}",
                    install_plan.catalog_release.flavor,
                    install_plan.catalog_release.release.tag_name
                );
                generate_compatibility_tool_vdf(
                    extracted_directory.join("compatibilitytool.vdf"),
                    &new_folder_name,
                    &format!(
                        "{} {}",
                        install_plan.catalog_release.flavor,
                        install_plan.catalog_release.release.tag_name
                    ),
                )
                .map_err(|err| format!("Failed to write compatibility tool VDF: {}", err))?;
                temp_dir.join(&new_folder_name)
            }
            CompatibilityToolFlavor::Unknown => {
                return Err("Unsupported compatibility tool flavor".to_string())
            }
        };

        if new_path != extracted_directory {
            std::fs::rename(extracted_directory, &new_path)
                .map_err(|err| format!("Failed to prepare compatibility tool layout: {}", err))?;
        }

        let target_directory_name = new_path
            .file_name()
            .ok_or_else(|| "Failed to resolve extracted compatibility tool name".to_string())?;
        let target_directory = compatibility_tools_directory.join(target_directory_name);

        if target_directory.exists() {
            return Err(format!(
                "Compatibility tool directory already exists: {}",
                target_directory.display()
            ));
        }

        std::fs::rename(&new_path, &target_directory).map_err(|err| {
            format!(
                "Failed to move compatibility tool into Steam compatibilitytools.d: {}",
                err
            )
        })?;

        Ok(format!(
            "Installation completed: {}",
            install_plan.catalog_release.release.tag_name
        ))
    }

    fn install_virtual_tool(
        &self,
        extracted_directory: &Path,
        install_plan: &InstallPlan,
        virtual_tool_id: &str,
    ) -> Result<String, String> {
        let manifest = self.load_virtual_tool_manifest();
        let Some(virtual_tool) = manifest
            .tools
            .iter()
            .find(|tool| tool.id == virtual_tool_id)
        else {
            return Err("Virtual compatibility tool no longer exists".to_string());
        };

        let target_directory = self
            .steam_util
            .get_steam_compatibility_tools_directory()
            .join(&virtual_tool.directory_name);

        let backup_directory = target_directory.with_file_name(format!(
            ".wine-cellar-backup-{}",
            virtual_tool.directory_name
        ));
        if backup_directory.exists() {
            recursive_delete_dir_entry(&backup_directory)
                .map_err(|err| format!("Failed to clear stale virtual tool backup: {}", err))?;
        }

        let mut backup_created = false;
        if target_directory.exists() {
            std::fs::rename(&target_directory, &backup_directory).map_err(|err| {
                format!(
                    "Failed to prepare virtual tool contents for replacement: {}",
                    err
                )
            })?;
            backup_created = true;
        }

        let install_result = (|| {
            std::fs::rename(extracted_directory, &target_directory).map_err(|err| {
                format!("Failed to move virtual tool contents into place: {}", err)
            })?;
            generate_compatibility_tool_vdf(
                target_directory.join("compatibilitytool.vdf"),
                &virtual_tool.steam_internal_name,
                &virtual_tool.user_label,
            )
            .map_err(|err| format!("Failed to write virtual tool VDF: {}", err))?;

            self.update_virtual_tool_payload(
                virtual_tool_id,
                Some(install_plan.catalog_release.id.clone()),
            )
        })();

        if let Err(err) = install_result {
            if target_directory.exists() {
                if let Err(cleanup_err) = recursive_delete_dir_entry(&target_directory) {
                    warn!(
                        "Failed to clean up incomplete virtual tool payload: {}",
                        cleanup_err
                    );
                }
            }
            if backup_created {
                if let Err(rollback_err) = std::fs::rename(&backup_directory, &target_directory) {
                    return Err(format!(
                        "{}; failed to restore previous virtual tool contents: {}",
                        err, rollback_err
                    ));
                }
            }
            return Err(err);
        }

        if backup_created {
            if let Err(err) = recursive_delete_dir_entry(&backup_directory) {
                warn!("Failed to remove virtual tool backup: {}", err);
            }
        }

        Ok(format!(
            "Mounted {} into {}",
            install_plan.catalog_release.release.tag_name, virtual_tool.user_label
        ))
    }
}

fn safe_unpack_tar(reader: Box<dyn Read>, destination: &Path) -> Result<(), String> {
    let mut archive = tar::Archive::new(reader);

    let entries = archive
        .entries()
        .map_err(|err| format!("Failed to read tar entries: {}", err))?;

    let mut entry_count = 0usize;
    let mut total_unpacked_size = 0u64;
    for entry in entries {
        let mut entry = entry.map_err(|err| format!("Failed to process tar entry: {}", err))?;
        entry_count += 1;
        if entry_count > MAX_ARCHIVE_ENTRIES {
            return Err("Archive contains too many entries".to_string());
        }

        let entry_type = entry.header().entry_type();
        if entry_type.is_block_special()
            || entry_type.is_character_special()
            || entry_type.is_fifo()
        {
            return Err("Archive contains unsupported special entries".to_string());
        }

        let path = entry
            .path()
            .map_err(|err| format!("Failed to resolve tar entry path: {}", err))?
            .into_owned();

        if !archive_path_is_safe(&path) {
            return Err(format!(
                "Unsafe archive entry path detected: {}",
                path.display()
            ));
        }

        if entry_type.is_symlink() {
            let target = entry
                .link_name()
                .map_err(|err| format!("Failed to resolve symbolic link target: {}", err))?
                .ok_or_else(|| "Archive symbolic link has no target".to_string())?;
            if !archive_symlink_target_is_safe(&path, &target) {
                return Err(format!(
                    "Unsafe archive symbolic link detected: {} -> {}",
                    path.display(),
                    target.display()
                ));
            }
        } else if entry_type.is_hard_link() {
            let target = entry
                .link_name()
                .map_err(|err| format!("Failed to resolve hard link target: {}", err))?
                .ok_or_else(|| "Archive hard link has no target".to_string())?;
            if !archive_relative_target_is_safe(&target, 0) {
                return Err(format!(
                    "Unsafe archive hard link detected: {} -> {}",
                    path.display(),
                    target.display()
                ));
            }
        }

        total_unpacked_size = total_unpacked_size
            .checked_add(entry.size())
            .ok_or_else(|| "Archive unpacked size overflowed".to_string())?;
        if total_unpacked_size > MAX_EXTRACTED_SIZE_BYTES {
            return Err("Archive unpacked size is larger than the supported limit".to_string());
        }

        let unpacked = entry
            .unpack_in(destination)
            .map_err(|err| format!("Failed to unpack archive entry: {}", err))?;
        if !unpacked {
            return Err(format!(
                "Archive entry could not be unpacked safely: {}",
                path.display()
            ));
        }
    }

    validate_extracted_symlinks(destination)
}

fn archive_path_is_safe(path: &Path) -> bool {
    !path.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    })
}

fn archive_symlink_target_is_safe(entry_path: &Path, target: &Path) -> bool {
    // Symbolic link targets are relative to the link's parent. Track the
    // lexical depth inside the archive root and reject targets that climb out
    // of it. Entry parents are already guaranteed to contain only safe,
    // relative components.
    let depth = entry_path
        .parent()
        .into_iter()
        .flat_map(Path::components)
        .filter(|component| matches!(component, Component::Normal(_)))
        .count();

    archive_relative_target_is_safe(target, depth)
}

fn archive_relative_target_is_safe(target: &Path, mut depth: usize) -> bool {
    if target.as_os_str().is_empty() {
        return false;
    }

    for component in target.components() {
        match component {
            Component::Normal(_) => depth += 1,
            Component::CurDir => {}
            Component::ParentDir if depth > 0 => depth -= 1,
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return false,
        }
    }

    true
}

fn validate_extracted_symlinks(destination: &Path) -> Result<(), String> {
    let canonical_destination = destination
        .canonicalize()
        .map_err(|err| format!("Failed to resolve extraction directory: {}", err))?;
    let mut pending_directories = vec![destination.to_path_buf()];

    while let Some(directory) = pending_directories.pop() {
        let entries = std::fs::read_dir(&directory)
            .map_err(|err| format!("Failed to audit extracted directory: {}", err))?;
        for entry in entries {
            let entry = entry.map_err(|err| format!("Failed to audit extracted entry: {}", err))?;
            let file_type = entry
                .file_type()
                .map_err(|err| format!("Failed to inspect extracted entry: {}", err))?;
            let path = entry.path();

            if file_type.is_symlink() {
                let resolved = path.canonicalize().map_err(|err| {
                    format!(
                        "Extracted symbolic link is dangling or invalid: {} ({})",
                        path.display(),
                        err
                    )
                })?;
                if !resolved.starts_with(&canonical_destination) {
                    return Err(format!(
                        "Extracted symbolic link escapes the archive: {} -> {}",
                        path.display(),
                        resolved.display()
                    ));
                }
            } else if file_type.is_dir() {
                pending_directories.push(path);
            }
        }
    }

    Ok(())
}

fn prepare_temp_directory(staging_root: &Path) -> Option<PathBuf> {
    let temp_dir = staging_root.join("temp");

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

fn download_archive_name(compression_type: &CompressionType) -> &'static str {
    match compression_type {
        CompressionType::Gzip => "download.tar.gz",
        CompressionType::Xz => "download.tar.xz",
        CompressionType::Unknown => "download.tar",
    }
}

fn cleanup_temp_directory(temp_dir: &Path) {
    if let Err(err) = recursive_delete_dir_entry(temp_dir) {
        error!("Failed to clean up temp directory: {}", err);
    }
}

fn look_for_compressed_archive(release: &Release) -> Option<DownloadPlan> {
    let is_supported_archive = |asset: &Asset| {
        let name = asset.name.to_ascii_lowercase();
        asset.content_type == "application/gzip"
            || asset.content_type == "application/x-xz"
            || name.ends_with(".tar.gz")
            || name.ends_with(".tar.xz")
    };

    let compression_type = |asset: &Asset| {
        let name = asset.name.to_ascii_lowercase();
        if name.ends_with(".tar.gz") {
            CompressionType::Gzip
        } else if name.ends_with(".tar.xz") {
            CompressionType::Xz
        } else if asset.content_type == "application/gzip" {
            CompressionType::Gzip
        } else if asset.content_type == "application/x-xz" {
            CompressionType::Xz
        } else {
            CompressionType::Unknown
        }
    };

    release
        .assets
        .iter()
        .filter(|asset| is_supported_archive(asset))
        .filter(|asset| is_steam_deck_archive(asset))
        .map(|asset| DownloadPlan {
            url: asset.browser_download_url.clone(),
            compression_type: compression_type(asset),
            expected_size: asset.size,
        })
        .next()
}

fn is_steam_deck_archive(asset: &Asset) -> bool {
    let name = asset.name.to_ascii_lowercase();
    ![
        "aarch64", "arm64", "armv7", "armhf", "riscv64", "ppc64", "s390x", "loong64",
    ]
    .iter()
    .any(|arch| name.contains(arch))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use tar::{Builder, EntryType, Header};
    use tempfile::tempdir;

    #[test]
    fn archive_path_safety_rejects_parent_and_absolute_paths() {
        assert!(!archive_path_is_safe(Path::new("../evil")));
        assert!(!archive_path_is_safe(Path::new("tool/../../evil")));
        assert!(!archive_path_is_safe(Path::new("/evil")));
        assert!(archive_path_is_safe(Path::new("tool/file")));
    }

    #[test]
    fn safe_unpack_tar_accepts_internal_symlink_entries() {
        let archive = archive_with_file_and_symlinks(
            "tool/lib/payload",
            &[
                ("tool/lib64", "lib"),
                ("tool/bin/payload", "../lib/payload"),
            ],
        );
        let destination = tempdir().expect("Failed to create temp directory");

        safe_unpack_tar(Box::new(Cursor::new(archive)), destination.path())
            .expect("Expected internal symbolic link to be extracted");

        assert_eq!(
            std::fs::read_link(destination.path().join("tool/lib64"))
                .expect("Expected extracted symbolic link"),
            PathBuf::from("lib")
        );
        assert_eq!(
            std::fs::read_link(destination.path().join("tool/bin/payload"))
                .expect("Expected extracted symbolic link"),
            PathBuf::from("../lib/payload")
        );
    }

    #[test]
    fn safe_unpack_tar_rejects_symlink_target_outside_archive() {
        for target in ["../../outside", "/etc/passwd"] {
            let archive = archive_with_symlink("tool/link", target);
            let destination = tempdir().expect("Failed to create temp directory");

            let err = safe_unpack_tar(Box::new(Cursor::new(archive)), destination.path())
                .expect_err("Expected escaping symbolic link to be rejected");

            assert!(err.contains("Unsafe archive symbolic link"));
        }
    }

    #[test]
    fn safe_unpack_tar_rejects_dangling_symlink() {
        let archive =
            archive_with_file_and_symlinks("tool/file", &[("tool/dangling", "missing-target")]);
        let destination = tempdir().expect("Failed to create temp directory");

        let err = safe_unpack_tar(Box::new(Cursor::new(archive)), destination.path())
            .expect_err("Expected dangling symbolic link to be rejected");

        assert!(err.contains("dangling or invalid"));
    }

    #[test]
    fn safe_unpack_tar_rejects_chained_symlink_escape() {
        // Each raw target is lexically inside the archive, but resolving `dir`
        // before the following `..` makes `escape` point outside the root.
        let archive =
            archive_with_file_and_symlinks("tool/file", &[("dir", "."), ("escape", "dir/..")]);
        let destination = tempdir().expect("Failed to create temp directory");

        let err = safe_unpack_tar(Box::new(Cursor::new(archive)), destination.path())
            .expect_err("Expected chained symbolic link escape to be rejected");

        assert!(err.contains("escapes the archive"));
    }

    #[test]
    fn safe_unpack_tar_accepts_internal_hard_link_entries() {
        let archive = archive_with_hard_link("tool/original", "tool/copy", "tool/../tool/original");
        let destination = tempdir().expect("Failed to create temp directory");

        safe_unpack_tar(Box::new(Cursor::new(archive)), destination.path())
            .expect("Expected internal hard link to be extracted");

        assert_eq!(
            std::fs::read(destination.path().join("tool/copy"))
                .expect("Expected extracted hard link"),
            b"payload"
        );
    }

    #[test]
    fn safe_unpack_tar_rejects_hard_link_target_outside_archive() {
        let archive = archive_with_hard_link("tool/original", "tool/copy", "../outside");
        let destination = tempdir().expect("Failed to create temp directory");

        let err = safe_unpack_tar(Box::new(Cursor::new(archive)), destination.path())
            .expect_err("Expected escaping hard link to be rejected");

        assert!(err.contains("Unsafe archive hard link"));
    }

    #[test]
    fn archive_selection_skips_aarch64_build_when_generic_build_exists() {
        let release = release_with_assets(vec![
            asset(
                "GE-Proton11-1-aarch64.tar.gz",
                "https://example.com/aarch64",
            ),
            asset("GE-Proton11-1.tar.gz", "https://example.com/x86_64"),
        ]);

        let plan = look_for_compressed_archive(&release).expect("Expected x86-compatible archive");

        assert_eq!(plan.url, "https://example.com/x86_64");
        assert_eq!(plan.compression_type, CompressionType::Gzip);
    }

    #[test]
    fn archive_selection_rejects_release_with_only_aarch64_build() {
        let release = release_with_assets(vec![asset(
            "GE-Proton11-1-aarch64.tar.gz",
            "https://example.com/aarch64",
        )]);

        assert!(look_for_compressed_archive(&release).is_none());
    }

    #[test]
    fn archive_selection_accepts_explicit_x86_64_build() {
        let release = release_with_assets(vec![asset(
            "proton-cachyos-10-x86_64_v3.tar.xz",
            "https://example.com/x86_64_v3",
        )]);

        let plan = look_for_compressed_archive(&release).expect("Expected x86_64 archive");

        assert_eq!(plan.url, "https://example.com/x86_64_v3");
        assert_eq!(plan.compression_type, CompressionType::Xz);
    }

    fn archive_with_symlink(path: &str, target: &str) -> Vec<u8> {
        let mut archive = Vec::new();
        {
            let mut builder = Builder::new(&mut archive);
            let mut header = Header::new_gnu();
            header.set_entry_type(EntryType::Symlink);
            header
                .set_link_name(target)
                .expect("Failed to set symlink target");
            header.set_size(0);
            header.set_cksum();
            builder
                .append_data(&mut header, path, std::io::empty())
                .expect("Failed to append symlink entry");
            builder.finish().expect("Failed to finish tar archive");
        }
        archive
    }

    fn archive_with_file_and_symlinks(file: &str, links: &[(&str, &str)]) -> Vec<u8> {
        let mut archive = Vec::new();
        {
            let mut builder = Builder::new(&mut archive);
            let mut file_header = Header::new_gnu();
            file_header.set_entry_type(EntryType::Regular);
            file_header.set_mode(0o644);
            file_header.set_size(7);
            file_header.set_cksum();
            builder
                .append_data(&mut file_header, file, Cursor::new(b"payload"))
                .expect("Failed to append regular file entry");

            for (path, target) in links {
                let mut link_header = Header::new_gnu();
                link_header.set_entry_type(EntryType::Symlink);
                link_header
                    .set_link_name(target)
                    .expect("Failed to set symlink target");
                link_header.set_size(0);
                link_header.set_cksum();
                builder
                    .append_data(&mut link_header, path, std::io::empty())
                    .expect("Failed to append symlink entry");
            }
            builder.finish().expect("Failed to finish tar archive");
        }
        archive
    }

    fn archive_with_hard_link(original: &str, link: &str, target: &str) -> Vec<u8> {
        let mut archive = Vec::new();
        {
            let mut builder = Builder::new(&mut archive);
            let mut file_header = Header::new_gnu();
            file_header.set_entry_type(EntryType::Regular);
            file_header.set_mode(0o644);
            file_header.set_size(7);
            file_header.set_cksum();
            builder
                .append_data(&mut file_header, original, Cursor::new(b"payload"))
                .expect("Failed to append regular file entry");

            let mut link_header = Header::new_gnu();
            link_header.set_entry_type(EntryType::Link);
            link_header
                .set_link_name(target)
                .expect("Failed to set hard link target");
            link_header.set_size(0);
            link_header.set_cksum();
            builder
                .append_data(&mut link_header, link, std::io::empty())
                .expect("Failed to append hard link entry");
            builder.finish().expect("Failed to finish tar archive");
        }
        archive
    }

    fn release_with_assets(assets: Vec<Asset>) -> Release {
        Release {
            url: "https://example.com/release".to_string(),
            id: 1,
            draft: false,
            prerelease: false,
            name: "Release".to_string(),
            tag_name: "Release".to_string(),
            assets,
            created_at: "2026-01-01T00:00:00Z".to_string(),
            published_at: "2026-01-01T00:00:00Z".to_string(),
            tarball_url: "https://example.com/source.tar.gz".to_string(),
            body: String::new(),
        }
    }

    fn asset(name: &str, url: &str) -> Asset {
        Asset {
            url: format!("{}/api", url),
            id: 1,
            name: name.to_string(),
            content_type: "application/gzip".to_string(),
            state: "uploaded".to_string(),
            size: 1024,
            download_count: 0,
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
            browser_download_url: url.to_string(),
        }
    }
}

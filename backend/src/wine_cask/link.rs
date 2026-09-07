use crate::wine_cask::app::WineCask;
use crate::wine_cask::flavors::{InstalledCompatibilityTool, InstalledToolSource};
use crate::wine_cask::recursive_delete_dir_entry;
use crate::wine_cask::virtual_tools::{
    ensure_virtual_tool_directory_accessible, write_virtualized_compatibility_tool_vdf,
    VirtualToolConfig,
};
use crate::PeerMap;
use log::{error, info, warn};
use std::ffi::OsStr;
use std::fs;
use std::io::ErrorKind;
use std::os::unix::fs::symlink;
use std::path::{Component, Path, PathBuf};

const COMPATIBILITY_TOOL_VDF: &str = "compatibilitytool.vdf";

pub(crate) fn can_link_source_path(
    compatibility_tools_directory: &Path,
    source_path: &Path,
) -> bool {
    validate_source_directory(compatibility_tools_directory, source_path).is_ok()
}

impl WineCask {
    pub fn reconcile_virtual_tool_payload_transactions(&self) -> Result<(), String> {
        let mut manifest = self.try_load_virtual_tool_manifest()?;
        if manifest.tools.is_empty() {
            return Ok(());
        }

        let base_directory = self
            .steam_util
            .get_steam_compatibility_tools_directory()
            .canonicalize()
            .map_err(|err| {
                format!(
                    "Failed to access compatibility tools directory during recovery: {}",
                    err
                )
            })?;
        let mut manifest_changed = false;

        for virtual_tool in &mut manifest.tools {
            let target_directory = immediate_child_path(
                &base_directory,
                &virtual_tool.directory_name,
                "virtual tool directory",
            )?;
            let backup_directory = immediate_child_path(
                &base_directory,
                &format!(".wine-cellar-backup-{}", virtual_tool.directory_name),
                "virtual tool backup directory",
            )?;
            let staging_directory = immediate_child_path(
                &base_directory,
                &format!(".wine-cellar-link-staging-{}", virtual_tool.directory_name),
                "virtual tool link staging directory",
            )?;

            match virtual_tool.pending_payload_transaction.take() {
                Some(transaction) => {
                    reconcile_pending_payload_transaction(
                        &base_directory,
                        &target_directory,
                        &backup_directory,
                        transaction.had_previous_target,
                    )?;
                    manifest_changed = true;
                }
                None => reconcile_completed_payload_transaction(
                    &base_directory,
                    &target_directory,
                    &backup_directory,
                )?,
            }

            clear_internal_directory(
                &staging_directory,
                "interrupted virtual tool link staging area",
            )?;
        }

        if manifest_changed {
            self.save_virtual_tool_manifest(&manifest)?;
        }
        Ok(())
    }

    pub async fn link_installed_tool_to_virtual_tool(
        &self,
        installed_tool_id: String,
        virtual_tool_id: String,
        peer_map: &PeerMap,
    ) {
        let result = self
            .link_installed_tool_to_virtual_tool_inner(&installed_tool_id, &virtual_tool_id)
            .await;

        match result {
            Ok(message) => {
                info!("{}", message);
                self.sync_backend_state().await;
                self.broadcast_app_state(peer_map).await;
                self.broadcast_notification(peer_map, &message).await;
            }
            Err(err) => {
                error!("Failed to link compatibility tool: {}", err);
                self.sync_backend_state().await;
                self.broadcast_app_state(peer_map).await;
                self.broadcast_notification(peer_map, &err).await;
            }
        }
    }

    async fn link_installed_tool_to_virtual_tool_inner(
        &self,
        installed_tool_id: &str,
        virtual_tool_id: &str,
    ) -> Result<String, String> {
        self.reconcile_virtual_tool_payload_transactions()?;
        let installed_tool = self
            .get_installed_tool(installed_tool_id)
            .await
            .ok_or_else(|| "Source compatibility tool no longer exists".to_string())?;
        if !matches!(&installed_tool.source, InstalledToolSource::Direct) {
            return Err("Only a directly installed compatibility tool can be linked".to_string());
        }

        let virtual_tool = self
            .try_load_virtual_tool_manifest()?
            .tools
            .into_iter()
            .find(|tool| tool.id == virtual_tool_id)
            .ok_or_else(|| "Virtual compatibility tool no longer exists".to_string())?;

        let compatibility_tools_directory =
            self.steam_util.get_steam_compatibility_tools_directory();
        let link_paths = validate_link_paths(
            &compatibility_tools_directory,
            &installed_tool,
            &virtual_tool,
        )?;

        validate_existing_target(
            &link_paths.base_directory,
            &link_paths.source_directory,
            &link_paths.target_directory,
        )?;
        recover_stale_backup(&link_paths.target_directory, &link_paths.backup_directory)?;
        validate_existing_target(
            &link_paths.base_directory,
            &link_paths.source_directory,
            &link_paths.target_directory,
        )?;
        self.ensure_virtual_tool_registration_compatible(
            &link_paths.source_directory.join(COMPATIBILITY_TOOL_VDF),
            Some(&installed_tool.internal_name),
            &virtual_tool,
        )
        .await?;
        clear_internal_directory(
            &link_paths.staging_directory,
            "stale virtual tool link staging area",
        )?;

        if let Err(err) = create_link_tree(
            &link_paths.source_directory,
            &installed_tool.directory_name,
            &installed_tool.internal_name,
            &link_paths.staging_directory,
            &virtual_tool,
        ) {
            if path_entry_exists(&link_paths.staging_directory).unwrap_or(false) {
                if let Err(cleanup_err) = recursive_delete_dir_entry(&link_paths.staging_directory)
                {
                    warn!(
                        "Failed to remove incomplete link staging area: {}",
                        cleanup_err
                    );
                }
            }
            return Err(err);
        }

        let backup_created = path_entry_exists(&link_paths.target_directory).map_err(|err| {
            format!(
                "Failed to inspect virtual tool contents before replacement: {}",
                err
            )
        })?;
        if let Err(err) =
            self.begin_virtual_tool_payload_transaction(virtual_tool_id, backup_created)
        {
            let _ = recursive_delete_dir_entry(&link_paths.staging_directory);
            return Err(format!(
                "Failed to record virtual tool replacement transaction: {}",
                err
            ));
        }

        if backup_created {
            if let Err(err) = fs::rename(&link_paths.target_directory, &link_paths.backup_directory)
            {
                let _ = recursive_delete_dir_entry(&link_paths.staging_directory);
                let clear_result = self.clear_virtual_tool_payload_transaction(virtual_tool_id);
                return Err(combine_rollback_error(
                    format!(
                        "Failed to prepare virtual tool contents for replacement: {}",
                        err
                    ),
                    clear_result.map_err(|clear_err| {
                        format!("failed to clear replacement transaction: {}", clear_err)
                    }),
                ));
            }
        }

        if let Err(err) = fs::rename(&link_paths.staging_directory, &link_paths.target_directory) {
            let restore_result = restore_backup(
                &link_paths.target_directory,
                &link_paths.backup_directory,
                backup_created,
            );
            let primary_error = format!(
                "Failed to move linked virtual tool contents into place: {}",
                err
            );
            return match restore_result {
                Ok(()) => Err(combine_rollback_error(
                    primary_error,
                    self.clear_virtual_tool_payload_transaction(virtual_tool_id)
                        .map_err(|clear_err| {
                            format!("failed to clear replacement transaction: {}", clear_err)
                        }),
                )),
                Err(rollback_error) => Err(format!(
                    "{}; {}; recovery transaction was preserved",
                    primary_error, rollback_error
                )),
            };
        }

        let metadata_result = self.update_virtual_tool_linked_payload(
            virtual_tool_id,
            installed_tool.id.clone(),
            installed_tool.directory_name.clone(),
            installed_tool.catalog_release_id.clone(),
            installed_tool.display_name.clone(),
            installed_tool.flavor.clone(),
        );
        if let Err(err) = metadata_result {
            let restore_result = restore_backup(
                &link_paths.target_directory,
                &link_paths.backup_directory,
                backup_created,
            );
            return match restore_result {
                Ok(()) => Err(combine_rollback_error(
                    err,
                    self.clear_virtual_tool_payload_transaction(virtual_tool_id)
                        .map_err(|clear_err| {
                            format!("failed to clear replacement transaction: {}", clear_err)
                        }),
                )),
                Err(rollback_error) => Err(format!(
                    "{}; {}; recovery transaction was preserved",
                    err, rollback_error
                )),
            };
        }

        if backup_created {
            if let Err(err) = recursive_delete_dir_entry(&link_paths.backup_directory) {
                warn!("Failed to remove virtual tool backup: {}", err);
            }
        }

        Ok(format!(
            "Linked {} into {}",
            installed_tool.display_name, virtual_tool.user_label
        ))
    }
}

#[derive(Debug)]
struct LinkPaths {
    base_directory: PathBuf,
    source_directory: PathBuf,
    target_directory: PathBuf,
    staging_directory: PathBuf,
    backup_directory: PathBuf,
}

fn validate_link_paths(
    compatibility_tools_directory: &Path,
    installed_tool: &InstalledCompatibilityTool,
    virtual_tool: &VirtualToolConfig,
) -> Result<LinkPaths, String> {
    let canonical_base = compatibility_tools_directory
        .canonicalize()
        .map_err(|err| {
            format!(
                "Failed to access compatibility tools base directory: {}",
                err
            )
        })?;
    let target_directory = immediate_child_path(
        &canonical_base,
        &virtual_tool.directory_name,
        "virtual tool directory",
    )?;
    let staging_directory = immediate_child_path(
        &canonical_base,
        &format!(".wine-cellar-link-staging-{}", virtual_tool.directory_name),
        "virtual tool link staging directory",
    )?;
    let backup_directory = immediate_child_path(
        &canonical_base,
        &format!(".wine-cellar-backup-{}", virtual_tool.directory_name),
        "virtual tool backup directory",
    )?;

    let source_path = PathBuf::from(&installed_tool.path);
    if source_path.file_name() != Some(OsStr::new(&installed_tool.directory_name)) {
        return Err("Source compatibility tool path no longer matches its identity".to_string());
    }
    let canonical_source = validate_source_directory(compatibility_tools_directory, &source_path)?;

    if canonical_source == target_directory {
        return Err("A virtual tool cannot link to itself".to_string());
    }

    Ok(LinkPaths {
        base_directory: canonical_base,
        source_directory: canonical_source,
        target_directory,
        staging_directory,
        backup_directory,
    })
}

fn validate_source_directory(
    compatibility_tools_directory: &Path,
    source_path: &Path,
) -> Result<PathBuf, String> {
    let canonical_base = compatibility_tools_directory
        .canonicalize()
        .map_err(|err| {
            format!(
                "Failed to access compatibility tools base directory: {}",
                err
            )
        })?;
    let source_metadata = fs::symlink_metadata(source_path)
        .map_err(|err| format!("Failed to inspect source compatibility tool: {}", err))?;
    if source_metadata.file_type().is_symlink() {
        return Err("A linked source compatibility tool cannot itself be a symlink".to_string());
    }
    if !source_metadata.is_dir() {
        return Err("Source compatibility tool is not a directory".to_string());
    }

    let canonical_source = source_path
        .canonicalize()
        .map_err(|err| format!("Failed to access source compatibility tool: {}", err))?;
    if canonical_source.parent() != Some(canonical_base.as_path()) {
        return Err(
            "Refusing to link a source outside the compatibility tools directory".to_string(),
        );
    }
    if !canonical_source.is_dir() {
        return Err("Source compatibility tool is not a directory".to_string());
    }

    Ok(canonical_source)
}

fn immediate_child_path(
    base_directory: &Path,
    name: &str,
    description: &str,
) -> Result<PathBuf, String> {
    let mut components = Path::new(name).components();
    if !matches!(components.next(), Some(Component::Normal(_))) || components.next().is_some() {
        return Err(format!("Invalid {} in virtual tool manifest", description));
    }

    let path = base_directory.join(name);
    if path.parent() != Some(base_directory) {
        return Err(format!("Invalid {} in virtual tool manifest", description));
    }
    Ok(path)
}

fn validate_existing_target(
    canonical_base: &Path,
    canonical_source: &Path,
    target_directory: &Path,
) -> Result<(), String> {
    if !path_entry_exists(target_directory)
        .map_err(|err| format!("Failed to inspect virtual tool directory: {}", err))?
    {
        return Ok(());
    }

    let metadata = fs::symlink_metadata(target_directory)
        .map_err(|err| format!("Failed to inspect virtual tool directory: {}", err))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err("Virtual tool path is not a safe directory".to_string());
    }

    let canonical_target = target_directory
        .canonicalize()
        .map_err(|err| format!("Failed to access virtual tool directory: {}", err))?;
    if canonical_target.parent() != Some(canonical_base) {
        return Err("Refusing to replace a virtual tool outside compatibilitytools.d".to_string());
    }
    if canonical_target == canonical_source {
        return Err("A virtual tool cannot link to itself".to_string());
    }

    Ok(())
}

fn recover_stale_backup(target_directory: &Path, backup_directory: &Path) -> Result<(), String> {
    if !path_entry_exists(backup_directory)
        .map_err(|err| format!("Failed to inspect virtual tool backup: {}", err))?
    {
        return Ok(());
    }

    if path_entry_exists(target_directory)
        .map_err(|err| format!("Failed to inspect virtual tool directory: {}", err))?
    {
        let target_metadata = fs::symlink_metadata(target_directory)
            .map_err(|err| format!("Failed to inspect virtual tool directory: {}", err))?;
        if target_metadata.file_type().is_symlink() || !target_metadata.is_dir() {
            return Err(
                "Refusing to discard a virtual tool backup for an invalid target".to_string(),
            );
        }
        return clear_internal_directory(backup_directory, "stale virtual tool backup");
    }

    let metadata = fs::symlink_metadata(backup_directory)
        .map_err(|err| format!("Failed to inspect virtual tool backup: {}", err))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err("Refusing to restore an invalid virtual tool backup".to_string());
    }

    fs::rename(backup_directory, target_directory).map_err(|err| {
        format!(
            "Failed to restore interrupted virtual tool replacement: {}",
            err
        )
    })
}

fn reconcile_pending_payload_transaction(
    base_directory: &Path,
    target_directory: &Path,
    backup_directory: &Path,
    had_previous_target: bool,
) -> Result<(), String> {
    let target_exists = path_entry_exists(target_directory)
        .map_err(|err| format!("Failed to inspect virtual tool during recovery: {}", err))?;
    let backup_exists = path_entry_exists(backup_directory).map_err(|err| {
        format!(
            "Failed to inspect virtual tool backup during recovery: {}",
            err
        )
    })?;

    if had_previous_target {
        if backup_exists {
            validate_recovery_directory(base_directory, backup_directory, "virtual tool backup")?;
            if target_exists {
                recursive_delete_dir_entry(target_directory).map_err(|err| {
                    format!("Failed to remove uncommitted virtual tool payload: {}", err)
                })?;
            }
            fs::rename(backup_directory, target_directory).map_err(|err| {
                format!(
                    "Failed to restore virtual tool payload during recovery: {}",
                    err
                )
            })?;
        } else if !target_exists {
            return Err(
                "Cannot recover interrupted virtual tool replacement: payload and backup are missing"
                    .to_string(),
            );
        }
    } else {
        if backup_exists {
            return Err(
                "Cannot recover interrupted virtual tool replacement: unexpected backup"
                    .to_string(),
            );
        }
        if target_exists {
            recursive_delete_dir_entry(target_directory).map_err(|err| {
                format!("Failed to remove uncommitted virtual tool payload: {}", err)
            })?;
        }
    }

    Ok(())
}

fn reconcile_completed_payload_transaction(
    base_directory: &Path,
    target_directory: &Path,
    backup_directory: &Path,
) -> Result<(), String> {
    if !path_entry_exists(backup_directory)
        .map_err(|err| format!("Failed to inspect stale virtual tool backup: {}", err))?
    {
        return Ok(());
    }

    validate_recovery_directory(base_directory, backup_directory, "virtual tool backup")?;
    if path_entry_exists(target_directory)
        .map_err(|err| format!("Failed to inspect virtual tool during recovery: {}", err))?
    {
        validate_recovery_directory(base_directory, target_directory, "virtual tool")?;
        recursive_delete_dir_entry(backup_directory)
            .map_err(|err| format!("Failed to remove committed virtual tool backup: {}", err))
    } else {
        fs::rename(backup_directory, target_directory).map_err(|err| {
            format!(
                "Failed to restore virtual tool backup with no active payload: {}",
                err
            )
        })
    }
}

fn validate_recovery_directory(
    base_directory: &Path,
    directory: &Path,
    description: &str,
) -> Result<(), String> {
    let metadata = fs::symlink_metadata(directory)
        .map_err(|err| format!("Failed to inspect {}: {}", description, err))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(format!("Refusing to recover an invalid {}", description));
    }
    let canonical_directory = directory
        .canonicalize()
        .map_err(|err| format!("Failed to access {}: {}", description, err))?;
    if canonical_directory.parent() != Some(base_directory) {
        return Err(format!(
            "Refusing to recover {} outside compatibilitytools.d",
            description
        ));
    }
    Ok(())
}

fn clear_internal_directory(path: &Path, description: &str) -> Result<(), String> {
    if path_entry_exists(path)
        .map_err(|err| format!("Failed to inspect {}: {}", description, err))?
    {
        recursive_delete_dir_entry(path)
            .map_err(|err| format!("Failed to clear {}: {}", description, err))?;
    }
    Ok(())
}

fn create_link_tree(
    source_directory: &Path,
    source_directory_name: &str,
    source_internal_name: &str,
    staging_directory: &Path,
    virtual_tool: &VirtualToolConfig,
) -> Result<(), String> {
    fs::create_dir(staging_directory)
        .map_err(|err| format!("Failed to create virtual tool link staging area: {}", err))?;

    let mut linked_entries = 0usize;
    for entry in fs::read_dir(source_directory)
        .map_err(|err| format!("Failed to read source compatibility tool: {}", err))?
    {
        let entry = entry
            .map_err(|err| format!("Failed to read source compatibility tool entry: {}", err))?;
        let file_name = entry.file_name();
        if file_name == OsStr::new(COMPATIBILITY_TOOL_VDF) {
            continue;
        }

        let relative_source = PathBuf::from("..")
            .join(source_directory_name)
            .join(&file_name);
        symlink(&relative_source, staging_directory.join(&file_name)).map_err(|err| {
            format!(
                "Failed to link compatibility tool entry {}: {}",
                entry.path().display(),
                err
            )
        })?;
        linked_entries += 1;
    }

    if linked_entries == 0 {
        return Err("Source compatibility tool contains no payload to link".to_string());
    }

    write_virtualized_compatibility_tool_vdf(
        &source_directory.join(COMPATIBILITY_TOOL_VDF),
        &staging_directory.join(COMPATIBILITY_TOOL_VDF),
        Some(source_internal_name),
        &virtual_tool.steam_internal_name,
        &virtual_tool.user_label,
    )
    .map_err(|err| format!("Failed to write virtual tool VDF: {}", err))?;

    ensure_virtual_tool_directory_accessible(staging_directory)
}

fn restore_backup(
    target_directory: &Path,
    backup_directory: &Path,
    backup_created: bool,
) -> Result<(), String> {
    if path_entry_exists(target_directory)
        .map_err(|err| format!("failed to inspect incomplete linked payload: {}", err))?
    {
        recursive_delete_dir_entry(target_directory)
            .map_err(|err| format!("failed to remove incomplete linked payload: {}", err))?;
    }

    if backup_created {
        fs::rename(backup_directory, target_directory)
            .map_err(|err| format!("failed to restore previous virtual tool contents: {}", err))?;
    }

    Ok(())
}

fn path_entry_exists(path: &Path) -> std::io::Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(false),
        Err(err) => Err(err),
    }
}

fn combine_rollback_error(error: String, rollback_result: Result<(), String>) -> String {
    match rollback_result {
        Ok(()) => error,
        Err(rollback_error) => format!("{}; {}", error, rollback_error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wine_cask::flavors::CompatibilityToolFlavor;
    use std::os::unix::fs::PermissionsExt;
    use tempfile::tempdir;

    fn virtual_tool() -> VirtualToolConfig {
        VirtualToolConfig {
            id: "virtual-1".to_string(),
            user_label: "Stable Proton".to_string(),
            steam_internal_name: "WineCellarVirtual1".to_string(),
            directory_name: "WineCellarVirtual1".to_string(),
            current_payload_release_id: None,
            current_payload_name: None,
            current_payload_flavor: Some(CompatibilityToolFlavor::Unknown),
            linked_source_installed_tool_id: None,
            linked_source_directory_name: None,
            pending_payload_transaction: None,
        }
    }

    #[test]
    fn creates_thin_link_tree_with_an_independent_vdf() {
        let directory = tempdir().unwrap();
        let source = directory.path().join("GE-Proton");
        let staging = directory.path().join(".wine-cellar-link-staging");
        fs::create_dir(&source).unwrap();
        fs::write(
            source.join(COMPATIBILITY_TOOL_VDF),
            r#""compatibilitytools"
            {
                "compat_tools"
                {
                    "SourceTool"
                    {
                        "install_path" "."
                        "display_name" "Source Tool"
                        "from_oslist" "windows"
                        "to_oslist" "linux"
                    }
                }
            }"#,
        )
        .unwrap();
        fs::write(source.join("proton"), "launcher").unwrap();
        fs::create_dir(source.join("files")).unwrap();
        fs::write(source.join("files").join("payload"), "payload").unwrap();
        symlink("files", source.join("current")).unwrap();
        fs::set_permissions(&source, fs::Permissions::from_mode(0o700)).unwrap();

        create_link_tree(
            &source,
            "GE-Proton",
            "SourceTool",
            &staging,
            &virtual_tool(),
        )
        .unwrap();

        assert_eq!(
            fs::read_link(staging.join("proton")).unwrap(),
            PathBuf::from("../GE-Proton/proton")
        );
        assert_eq!(
            fs::read_link(staging.join("files")).unwrap(),
            PathBuf::from("../GE-Proton/files")
        );
        assert_eq!(
            fs::read_link(staging.join("current")).unwrap(),
            PathBuf::from("../GE-Proton/current")
        );
        assert_eq!(
            fs::read_to_string(staging.join("proton")).unwrap(),
            "launcher"
        );
        assert_eq!(
            fs::read_to_string(staging.join("current").join("payload")).unwrap(),
            "payload"
        );

        let vdf_metadata = fs::symlink_metadata(staging.join(COMPATIBILITY_TOOL_VDF)).unwrap();
        assert!(vdf_metadata.is_file());
        assert!(!vdf_metadata.file_type().is_symlink());
        let vdf = fs::read_to_string(staging.join(COMPATIBILITY_TOOL_VDF)).unwrap();
        assert!(vdf.contains("WineCellarVirtual1"));
        assert!(vdf.contains("Stable Proton"));
        assert_eq!(
            fs::metadata(&staging).unwrap().permissions().mode() & 0o055,
            0o055
        );
    }

    #[test]
    fn rollback_restores_previous_virtual_tool_contents() {
        let directory = tempdir().unwrap();
        let target = directory.path().join("WineCellarVirtual1");
        let backup = directory
            .path()
            .join(".wine-cellar-backup-WineCellarVirtual1");
        fs::create_dir(&target).unwrap();
        fs::write(target.join("new"), "new payload").unwrap();
        fs::create_dir(&backup).unwrap();
        fs::write(backup.join("old"), "old payload").unwrap();

        restore_backup(&target, &backup, true).unwrap();

        assert!(!target.join("new").exists());
        assert_eq!(
            fs::read_to_string(target.join("old")).unwrap(),
            "old payload"
        );
        assert!(!backup.exists());
    }

    #[test]
    fn pending_transaction_recovery_prefers_the_recorded_backup() {
        let directory = tempdir().unwrap();
        let target = directory.path().join("WineCellarVirtual1");
        let backup = directory
            .path()
            .join(".wine-cellar-backup-WineCellarVirtual1");
        fs::create_dir(&target).unwrap();
        fs::write(target.join("payload"), "uncommitted").unwrap();
        fs::create_dir(&backup).unwrap();
        fs::write(backup.join("payload"), "previous").unwrap();

        reconcile_pending_payload_transaction(directory.path(), &target, &backup, true).unwrap();

        assert_eq!(
            fs::read_to_string(target.join("payload")).unwrap(),
            "previous"
        );
        assert!(fs::symlink_metadata(backup).is_err());
    }

    #[test]
    fn pending_first_payload_recovery_removes_the_uncommitted_target() {
        let directory = tempdir().unwrap();
        let target = directory.path().join("WineCellarVirtual1");
        let backup = directory
            .path()
            .join(".wine-cellar-backup-WineCellarVirtual1");
        fs::create_dir(&target).unwrap();
        fs::write(target.join("payload"), "uncommitted").unwrap();

        reconcile_pending_payload_transaction(directory.path(), &target, &backup, false).unwrap();

        assert!(fs::symlink_metadata(target).is_err());
        assert!(fs::symlink_metadata(backup).is_err());
    }

    #[test]
    fn refuses_sources_outside_the_compatibility_tools_directory() {
        let directory = tempdir().unwrap();
        let base = directory.path().join("compatibilitytools.d");
        let outside = directory.path().join("outside");
        fs::create_dir(&base).unwrap();
        fs::create_dir(&outside).unwrap();
        let installed_tool = InstalledCompatibilityTool {
            id: "installed:outside".to_string(),
            path: outside.to_string_lossy().to_string(),
            directory_name: "outside".to_string(),
            display_name: "Outside".to_string(),
            internal_name: "Outside".to_string(),
            used_by_games: Vec::new(),
            requires_restart: false,
            flavor: CompatibilityToolFlavor::Unknown,
            catalog_release_id: None,
            github_release: None,
            source: InstalledToolSource::Direct,
            virtual_tool_id: None,
            user_label: None,
            can_link_to_virtual_tool: true,
        };

        let error = validate_link_paths(&base, &installed_tool, &virtual_tool()).unwrap_err();
        assert!(error.contains("outside the compatibility tools directory"));
    }

    #[test]
    fn refuses_virtual_tool_directory_path_traversal() {
        let directory = tempdir().unwrap();
        let base = directory.path().join("compatibilitytools.d");
        let source = base.join("GE-Proton");
        fs::create_dir_all(&source).unwrap();
        let installed_tool = InstalledCompatibilityTool {
            id: "installed:GE-Proton".to_string(),
            path: source.to_string_lossy().to_string(),
            directory_name: "GE-Proton".to_string(),
            display_name: "GE-Proton".to_string(),
            internal_name: "GE-Proton".to_string(),
            used_by_games: Vec::new(),
            requires_restart: false,
            flavor: CompatibilityToolFlavor::ProtonGE,
            catalog_release_id: None,
            github_release: None,
            source: InstalledToolSource::Direct,
            virtual_tool_id: None,
            user_label: None,
            can_link_to_virtual_tool: true,
        };
        let mut unsafe_virtual_tool = virtual_tool();
        unsafe_virtual_tool.directory_name = "../escaped".to_string();

        let error = validate_link_paths(&base, &installed_tool, &unsafe_virtual_tool).unwrap_err();
        assert!(error.contains("Invalid virtual tool directory"));
        assert!(!directory.path().join("escaped").exists());
    }

    #[test]
    fn refuses_a_root_symlink_as_the_link_source() {
        let directory = tempdir().unwrap();
        let base = directory.path().join("compatibilitytools.d");
        let actual_source = base.join("Actual-Proton");
        let source_alias = base.join("Alias-Proton");
        fs::create_dir_all(&actual_source).unwrap();
        symlink("Actual-Proton", &source_alias).unwrap();
        let installed_tool = InstalledCompatibilityTool {
            id: "installed:Alias-Proton".to_string(),
            path: source_alias.to_string_lossy().to_string(),
            directory_name: "Alias-Proton".to_string(),
            display_name: "Alias Proton".to_string(),
            internal_name: "AliasProton".to_string(),
            used_by_games: Vec::new(),
            requires_restart: false,
            flavor: CompatibilityToolFlavor::Unknown,
            catalog_release_id: None,
            github_release: None,
            source: InstalledToolSource::Direct,
            virtual_tool_id: None,
            user_label: None,
            can_link_to_virtual_tool: false,
        };

        assert!(can_link_source_path(&base, &actual_source));
        assert!(!can_link_source_path(&base, &source_alias));
        let error = validate_link_paths(&base, &installed_tool, &virtual_tool()).unwrap_err();
        assert!(error.contains("cannot itself be a symlink"));
    }

    #[test]
    fn recovers_backup_left_before_target_swap() {
        let directory = tempdir().unwrap();
        let target = directory.path().join("WineCellarVirtual1");
        let backup = directory
            .path()
            .join(".wine-cellar-backup-WineCellarVirtual1");
        fs::create_dir(&backup).unwrap();
        fs::write(backup.join("payload"), "previous payload").unwrap();

        recover_stale_backup(&target, &backup).unwrap();

        assert_eq!(
            fs::read_to_string(target.join("payload")).unwrap(),
            "previous payload"
        );
        assert!(!backup.exists());
    }

    #[test]
    fn unsafe_target_does_not_discard_recoverable_backup() {
        let directory = tempdir().unwrap();
        let outside = directory.path().join("outside");
        let target = directory.path().join("WineCellarVirtual1");
        let backup = directory
            .path()
            .join(".wine-cellar-backup-WineCellarVirtual1");
        fs::create_dir(&outside).unwrap();
        symlink(&outside, &target).unwrap();
        fs::create_dir(&backup).unwrap();
        fs::write(backup.join("payload"), "previous payload").unwrap();

        let error = recover_stale_backup(&target, &backup).unwrap_err();

        assert!(error.contains("invalid target"));
        assert_eq!(
            fs::read_to_string(backup.join("payload")).unwrap(),
            "previous payload"
        );
    }
}

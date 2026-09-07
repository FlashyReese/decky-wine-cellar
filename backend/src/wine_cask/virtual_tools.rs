use crate::wine_cask::app::WineCask;
use crate::wine_cask::flavors::CompatibilityToolFlavor;
use crate::wine_cask::generate_compatibility_tool_vdf;
use crate::wine_cask::recursive_delete_dir_entry;
use crate::PeerMap;
use keyvalues_parser::{Obj, Value, Vdf};
use log::{error, warn};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::fs;
use std::fs::{create_dir_all, OpenOptions};
use std::io::{ErrorKind, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_VIRTUAL_TOOL_LABEL_CHARS: usize = 64;

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct VirtualToolManifest {
    pub next_virtual_tool_number: u64,
    pub tools: Vec<VirtualToolConfig>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct VirtualToolConfig {
    pub id: String,
    pub user_label: String,
    pub steam_internal_name: String,
    pub directory_name: String,
    #[serde(default)]
    pub current_payload_release_id: Option<String>,
    #[serde(default)]
    pub current_payload_name: Option<String>,
    #[serde(default)]
    pub current_payload_flavor: Option<CompatibilityToolFlavor>,
    #[serde(default)]
    pub linked_source_installed_tool_id: Option<String>,
    #[serde(default)]
    pub linked_source_directory_name: Option<String>,
    #[serde(default)]
    pub pending_payload_transaction: Option<VirtualToolPayloadTransaction>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct VirtualToolPayloadTransaction {
    pub had_previous_target: bool,
}

impl VirtualToolConfig {
    fn set_catalog_payload(
        &mut self,
        release_id: String,
        payload_name: String,
        payload_flavor: CompatibilityToolFlavor,
    ) {
        self.current_payload_release_id = Some(release_id);
        self.current_payload_name = Some(payload_name);
        self.current_payload_flavor = Some(payload_flavor);
        self.linked_source_installed_tool_id = None;
        self.linked_source_directory_name = None;
        self.pending_payload_transaction = None;
    }

    fn set_linked_payload(
        &mut self,
        source_installed_tool_id: String,
        source_directory_name: String,
        release_id: Option<String>,
        payload_name: String,
        payload_flavor: CompatibilityToolFlavor,
    ) {
        self.current_payload_release_id = release_id;
        self.current_payload_name = Some(payload_name);
        self.current_payload_flavor = Some(payload_flavor);
        self.linked_source_installed_tool_id = Some(source_installed_tool_id);
        self.linked_source_directory_name = Some(source_directory_name);
        self.pending_payload_transaction = None;
    }

    fn links_source(&self, installed_tool_id: &str, directory_name: &str) -> bool {
        self.linked_source_installed_tool_id.as_deref() == Some(installed_tool_id)
            || self.linked_source_directory_name.as_deref() == Some(directory_name)
    }
}

impl VirtualToolManifest {
    fn next_tool_identity(&mut self) -> Result<(String, String, String), String> {
        let next_number = self.next_virtual_tool_number.max(1);
        self.next_virtual_tool_number = next_number
            .checked_add(1)
            .ok_or_else(|| "No more virtual tool identities are available".to_string())?;

        let id = format!("virtual-{}", next_number);
        let steam_internal_name = format!("WineCellarVirtual{}", next_number);

        Ok((id, steam_internal_name.clone(), steam_internal_name))
    }
}

impl WineCask {
    pub fn load_virtual_tool_manifest(&self) -> VirtualToolManifest {
        match self.try_load_virtual_tool_manifest() {
            Ok(manifest) => manifest,
            Err(err) => {
                warn!("{}", err);
                VirtualToolManifest {
                    next_virtual_tool_number: 1,
                    tools: Vec::new(),
                }
            }
        }
    }

    pub fn try_load_virtual_tool_manifest(&self) -> Result<VirtualToolManifest, String> {
        match fs::symlink_metadata(&self.virtual_tool_manifest_path) {
            Ok(_) => {}
            Err(err) if err.kind() == ErrorKind::NotFound => {
                return Ok(VirtualToolManifest {
                    next_virtual_tool_number: 1,
                    tools: Vec::new(),
                })
            }
            Err(err) => {
                return Err(format!("Failed to inspect virtual tool manifest: {}", err));
            }
        }

        let contents = fs::read_to_string(&self.virtual_tool_manifest_path)
            .map_err(|err| format!("Failed to read virtual tool manifest: {}", err))?;
        let mut manifest = serde_json::from_str::<VirtualToolManifest>(&contents)
            .map_err(|err| format!("Failed to parse virtual tool manifest: {}", err))?;
        if manifest.next_virtual_tool_number == 0 {
            manifest.next_virtual_tool_number = manifest.tools.len() as u64 + 1;
        }
        Ok(manifest)
    }

    pub fn save_virtual_tool_manifest(&self, manifest: &VirtualToolManifest) -> Result<(), String> {
        let manifest_parent = self
            .virtual_tool_manifest_path
            .parent()
            .ok_or_else(|| "Failed to resolve virtual tool manifest parent".to_string())?;

        create_dir_all(manifest_parent)
            .map_err(|err| format!("Failed to prepare virtual tool manifest directory: {}", err))?;

        let manifest_json = serde_json::to_string_pretty(manifest)
            .map_err(|err| format!("Failed to serialize virtual tool manifest: {}", err))?;

        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|err| format!("Failed to prepare virtual tool manifest temp name: {}", err))?
            .as_nanos();
        let temp_manifest_path = manifest_parent.join(format!(
            ".wine-cellar-virtual-tools-{}-{}.tmp",
            std::process::id(),
            nonce
        ));
        let mut temp_manifest = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_manifest_path)
            .map_err(|err| format!("Failed to create virtual tool manifest temp file: {}", err))?;
        let write_result = temp_manifest
            .write_all(manifest_json.as_bytes())
            .map_err(|err| format!("Failed to write virtual tool manifest temp file: {}", err))
            .and_then(|()| {
                temp_manifest.sync_all().map_err(|err| {
                    format!("Failed to flush virtual tool manifest temp file: {}", err)
                })
            });
        drop(temp_manifest);
        if let Err(err) = write_result {
            let _ = fs::remove_file(&temp_manifest_path);
            return Err(err);
        }

        if let Err(err) = fs::rename(&temp_manifest_path, &self.virtual_tool_manifest_path) {
            let _ = fs::remove_file(&temp_manifest_path);
            return Err(format!("Failed to persist virtual tool manifest: {}", err));
        }
        Ok(())
    }

    pub fn create_virtual_tool_slot(&self, user_label: String) -> Result<String, String> {
        let trimmed_label = normalize_virtual_tool_label(&user_label)?;

        let mut manifest = self.try_load_virtual_tool_manifest()?;
        let compatibility_tools_directory =
            self.steam_util.get_steam_compatibility_tools_directory();
        create_dir_all(&compatibility_tools_directory)
            .map_err(|err| format!("Failed to prepare compatibility tools directory: {}", err))?;
        let canonical_base = compatibility_tools_directory
            .canonicalize()
            .map_err(|err| format!("Failed to access compatibility tools directory: {}", err))?;

        let (id, steam_internal_name, directory_name, tool_dir) = loop {
            let (id, steam_internal_name, directory_name) = manifest.next_tool_identity()?;
            if manifest.tools.iter().any(|tool| {
                tool.id == id
                    || tool.steam_internal_name == steam_internal_name
                    || tool.directory_name == directory_name
            }) {
                continue;
            }

            let tool_dir = safe_manifest_child_path(
                &canonical_base,
                &directory_name,
                "virtual tool directory",
            )?;
            match fs::create_dir(&tool_dir) {
                Ok(()) => break (id, steam_internal_name, directory_name, tool_dir),
                Err(err) if err.kind() == ErrorKind::AlreadyExists => continue,
                Err(err) => {
                    return Err(format!("Failed to create virtual tool directory: {}", err));
                }
            }
        };

        if let Err(err) = ensure_virtual_tool_directory_accessible(&tool_dir) {
            let _ = recursive_delete_dir_entry(&tool_dir);
            return Err(err);
        }
        if let Err(err) = generate_compatibility_tool_vdf(
            tool_dir.join("compatibilitytool.vdf"),
            &steam_internal_name,
            &trimmed_label,
        ) {
            if let Err(cleanup_err) = recursive_delete_dir_entry(&tool_dir) {
                error!(
                    "Failed to roll back virtual tool directory after VDF write error: {}",
                    cleanup_err
                );
            }
            return Err(format!("Failed to write virtual tool VDF: {}", err));
        }
        if let Err(err) =
            ensure_virtual_tool_vdf_accessible(&tool_dir.join("compatibilitytool.vdf"))
        {
            let _ = recursive_delete_dir_entry(&tool_dir);
            return Err(err);
        }

        manifest.tools.push(VirtualToolConfig {
            id,
            user_label: trimmed_label.clone(),
            steam_internal_name,
            directory_name,
            current_payload_release_id: None,
            current_payload_name: None,
            current_payload_flavor: None,
            linked_source_installed_tool_id: None,
            linked_source_directory_name: None,
            pending_payload_transaction: None,
        });

        if let Err(err) = self.save_virtual_tool_manifest(&manifest) {
            if let Err(cleanup_err) = recursive_delete_dir_entry(&tool_dir) {
                error!(
                    "Failed to roll back virtual tool directory after manifest save error: {}",
                    cleanup_err
                );
            }
            return Err(err);
        }

        Ok(trimmed_label)
    }

    pub fn rename_virtual_tool_slot(
        &self,
        virtual_tool_id: &str,
        user_label: String,
    ) -> Result<String, String> {
        let trimmed_label = normalize_virtual_tool_label(&user_label)?;

        let mut manifest = self.try_load_virtual_tool_manifest()?;
        let Some(position) = manifest
            .tools
            .iter()
            .position(|tool| tool.id == virtual_tool_id)
        else {
            return Err("Virtual tool not found".to_string());
        };

        let original = manifest.tools[position].clone();
        let tool_dir = validate_virtual_tool_slot_directory(
            &self.steam_util.get_steam_compatibility_tools_directory(),
            &original.directory_name,
        )?;
        rewrite_virtual_tool_vdf(&tool_dir, &original.steam_internal_name, &trimmed_label)?;

        manifest.tools[position].user_label = trimmed_label.clone();
        if let Err(err) = self.save_virtual_tool_manifest(&manifest) {
            return match rewrite_virtual_tool_vdf(
                &tool_dir,
                &original.steam_internal_name,
                &original.user_label,
            ) {
                Ok(()) => Err(err),
                Err(rollback_err) => Err(format!(
                    "{}; failed to restore the previous virtual tool label: {}",
                    err, rollback_err
                )),
            };
        }

        Ok(trimmed_label)
    }

    pub fn update_virtual_tool_catalog_payload(
        &self,
        virtual_tool_id: &str,
        release_id: String,
        payload_name: String,
        payload_flavor: CompatibilityToolFlavor,
    ) -> Result<(), String> {
        let mut manifest = self.try_load_virtual_tool_manifest()?;
        let Some(config) = manifest
            .tools
            .iter_mut()
            .find(|tool| tool.id == virtual_tool_id)
        else {
            return Err("Virtual tool not found".to_string());
        };

        config.set_catalog_payload(release_id, payload_name, payload_flavor);
        self.save_virtual_tool_manifest(&manifest)
    }

    pub fn begin_virtual_tool_payload_transaction(
        &self,
        virtual_tool_id: &str,
        had_previous_target: bool,
    ) -> Result<(), String> {
        let mut manifest = self.try_load_virtual_tool_manifest()?;
        let Some(config) = manifest
            .tools
            .iter_mut()
            .find(|tool| tool.id == virtual_tool_id)
        else {
            return Err("Virtual tool not found".to_string());
        };

        config.pending_payload_transaction = Some(VirtualToolPayloadTransaction {
            had_previous_target,
        });
        self.save_virtual_tool_manifest(&manifest)
    }

    pub fn clear_virtual_tool_payload_transaction(
        &self,
        virtual_tool_id: &str,
    ) -> Result<(), String> {
        let mut manifest = self.try_load_virtual_tool_manifest()?;
        let Some(config) = manifest
            .tools
            .iter_mut()
            .find(|tool| tool.id == virtual_tool_id)
        else {
            return Err("Virtual tool not found".to_string());
        };

        config.pending_payload_transaction = None;
        self.save_virtual_tool_manifest(&manifest)
    }

    pub fn update_virtual_tool_linked_payload(
        &self,
        virtual_tool_id: &str,
        source_installed_tool_id: String,
        source_directory_name: String,
        release_id: Option<String>,
        payload_name: String,
        payload_flavor: CompatibilityToolFlavor,
    ) -> Result<(), String> {
        let mut manifest = self.try_load_virtual_tool_manifest()?;
        let Some(config) = manifest
            .tools
            .iter_mut()
            .find(|tool| tool.id == virtual_tool_id)
        else {
            return Err("Virtual tool not found".to_string());
        };

        config.set_linked_payload(
            source_installed_tool_id,
            source_directory_name,
            release_id,
            payload_name,
            payload_flavor,
        );
        self.save_virtual_tool_manifest(&manifest)
    }

    pub fn virtual_tools_linking_source(
        &self,
        installed_tool_id: &str,
        directory_name: &str,
    ) -> Result<Vec<String>, String> {
        let manifest = self.try_load_virtual_tool_manifest()?;
        let compatibility_tools_directory = self
            .steam_util
            .get_steam_compatibility_tools_directory()
            .canonicalize()
            .map_err(|err| {
                format!(
                    "Failed to verify compatibility tools directory identity: {}",
                    err
                )
            })?;
        let requested_source = safe_manifest_child_path(
            &compatibility_tools_directory,
            directory_name,
            "installed tool directory",
        )?
        .canonicalize()
        .map_err(|err| {
            format!(
                "Failed to verify installed tool directory identity: {}",
                err
            )
        })?;

        let mut linked_labels = Vec::new();
        for tool in manifest.tools {
            let directly_matches = tool.links_source(installed_tool_id, directory_name);
            let canonically_matches = match tool.linked_source_directory_name.as_deref() {
                Some(linked_directory_name) => {
                    let linked_path = safe_manifest_child_path(
                        &compatibility_tools_directory,
                        linked_directory_name,
                        "linked source directory",
                    )?;
                    match linked_path.canonicalize() {
                        Ok(linked_source) => linked_source == requested_source,
                        Err(err) if err.kind() == ErrorKind::NotFound => false,
                        Err(err) => {
                            return Err(format!(
                                "Failed to verify linked source directory identity: {}",
                                err
                            ))
                        }
                    }
                }
                None => false,
            };

            if directly_matches || canonically_matches {
                linked_labels.push(tool.user_label);
            }
        }

        Ok(linked_labels)
    }

    pub async fn ensure_virtual_tool_registration_compatible(
        &self,
        source_vdf: &Path,
        source_internal_name: Option<&str>,
        virtual_tool: &VirtualToolConfig,
    ) -> Result<(), String> {
        let tool_directory = validate_virtual_tool_slot_directory(
            &self.steam_util.get_steam_compatibility_tools_directory(),
            &virtual_tool.directory_name,
        )?;
        let current_contract = compatibility_tool_registration_contract(
            &tool_directory.join("compatibilitytool.vdf"),
            Some(&virtual_tool.steam_internal_name),
        )?;
        let source_contract =
            compatibility_tool_registration_contract(source_vdf, source_internal_name)?;
        if current_contract == source_contract {
            return Ok(());
        }

        let app_state = self.app_state.lock().await;
        let visibility_is_authoritative = !app_state.steam_visible_tools.is_empty();
        let slot_is_visible = app_state
            .steam_visible_tools
            .iter()
            .any(|tool| tool.str_tool_name == virtual_tool.steam_internal_name);
        if visibility_is_authoritative && !slot_is_visible {
            return Ok(());
        }

        Err(
            "The selected tool has a different Steam registration; create a new virtual slot, link or mount it there, then restart Steam"
                .to_string(),
        )
    }

    pub fn remove_virtual_tool_slot(&self, virtual_tool_id: &str) -> Result<String, String> {
        let mut manifest = self.try_load_virtual_tool_manifest()?;
        let Some(position) = manifest
            .tools
            .iter()
            .position(|tool| tool.id == virtual_tool_id)
        else {
            return Err("Virtual tool not found".to_string());
        };

        let removed_tool = manifest.tools.remove(position);
        self.save_virtual_tool_manifest(&manifest)?;
        Ok(removed_tool.user_label)
    }

    pub async fn remove_virtual_tool(&self, virtual_tool_id: String, peer_map: &PeerMap) {
        if let Err(err) = self.reconcile_virtual_tool_payload_transactions() {
            self.broadcast_notification(
                peer_map,
                &format!("Cannot safely remove virtual tool during recovery: {}", err),
            )
            .await;
            return;
        }
        let virtual_tool = match self.try_load_virtual_tool_manifest() {
            Ok(manifest) => manifest
                .tools
                .into_iter()
                .find(|tool| tool.id == virtual_tool_id),
            Err(err) => {
                self.broadcast_notification(
                    peer_map,
                    &format!("Cannot safely remove virtual tool: {}", err),
                )
                .await;
                return;
            }
        };
        let Some(virtual_tool) = virtual_tool else {
            self.broadcast_notification(peer_map, "Virtual tool not found")
                .await;
            return;
        };

        let compatibility_tools_directory =
            self.steam_util.get_steam_compatibility_tools_directory();
        if let Err(err) = remove_virtual_tool_reserved_directories(
            &compatibility_tools_directory,
            &virtual_tool.directory_name,
        ) {
            error!("{}", err);
            self.broadcast_notification(peer_map, &err).await;
            return;
        }

        if let Err(err) = self.remove_virtual_tool_slot(&virtual_tool_id) {
            let error_message = format!(
                "Compatibility tool directory removed but virtual tool manifest update failed: {}",
                err
            );
            error!("{}", error_message);
            self.broadcast_notification(peer_map, &error_message).await;
            return;
        }

        self.sync_backend_state().await;
        self.broadcast_app_state(peer_map).await;
        self.broadcast_notification(
            peer_map,
            &format!(
                "Removed virtual compatibility tool: {}",
                virtual_tool.user_label
            ),
        )
        .await;
    }
}

fn rewrite_virtual_tool_vdf(
    tool_dir: &std::path::Path,
    steam_internal_name: &str,
    user_label: &str,
) -> Result<(), String> {
    let vdf_path = tool_dir.join("compatibilitytool.vdf");
    write_virtualized_compatibility_tool_vdf(
        &vdf_path,
        &vdf_path,
        Some(steam_internal_name),
        steam_internal_name,
        user_label,
    )
}

pub(crate) fn write_virtualized_compatibility_tool_vdf(
    source_path: &Path,
    destination_path: &Path,
    source_internal_name: Option<&str>,
    target_internal_name: &str,
    target_display_name: &str,
) -> Result<(), String> {
    let source_mode = fs::metadata(source_path)
        .map_err(|err| format!("Failed to read source compatibility tool VDF: {}", err))?
        .permissions()
        .mode();
    let source_contents = fs::read_to_string(source_path)
        .map_err(|err| format!("Failed to read source compatibility tool VDF: {}", err))?;
    let vdf = virtualized_compatibility_tool_vdf(
        &source_contents,
        source_internal_name,
        target_internal_name,
        target_display_name,
    )?;

    let destination_parent = destination_path
        .parent()
        .ok_or_else(|| "Failed to resolve virtual tool VDF parent".to_string())?;
    let destination_parent_metadata = fs::symlink_metadata(destination_parent)
        .map_err(|err| format!("Failed to inspect virtual tool VDF directory: {}", err))?;
    if destination_parent_metadata.file_type().is_symlink() || !destination_parent_metadata.is_dir()
    {
        return Err("Virtual tool VDF parent is not a safe directory".to_string());
    }
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| format!("Failed to prepare virtual tool VDF temp name: {}", err))?
        .as_nanos();
    let temp_path = destination_parent.join(format!(
        ".wine-cellar-vdf-{}-{}.tmp",
        std::process::id(),
        nonce
    ));

    let write_result = (|| {
        let mut temp_file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)
            .map_err(|err| format!("Failed to create virtual tool VDF temp file: {}", err))?;
        temp_file
            .write_all(vdf.to_string().as_bytes())
            .map_err(|err| format!("Failed to write virtual tool VDF temp file: {}", err))?;
        temp_file
            .sync_all()
            .map_err(|err| format!("Failed to flush virtual tool VDF temp file: {}", err))?;
        drop(temp_file);
        fs::set_permissions(&temp_path, fs::Permissions::from_mode(source_mode | 0o044))
            .map_err(|err| format!("Failed to set compatibility tool VDF mode: {}", err))?;
        fs::rename(&temp_path, destination_path)
            .map_err(|err| format!("Failed to install virtual tool VDF: {}", err))
    })();

    if write_result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }
    write_result
}

fn virtualized_compatibility_tool_vdf<'a>(
    source_contents: &'a str,
    source_internal_name: Option<&str>,
    target_internal_name: &str,
    target_display_name: &str,
) -> Result<Vdf<'a>, String> {
    let mut vdf = Vdf::parse(source_contents)
        .map_err(|err| format!("Failed to parse source compatibility tool VDF: {}", err))?;
    let root = vdf
        .value
        .get_mut_obj()
        .ok_or_else(|| "Source compatibility tool VDF root is not an object".to_string())?;
    let compat_tool_blocks = root
        .get_mut("compat_tools")
        .ok_or_else(|| "Source compatibility tool VDF has no compat_tools object".to_string())?;

    let candidates: Vec<(usize, String)> = compat_tool_blocks
        .iter()
        .enumerate()
        .filter_map(|(block_index, value)| {
            value.get_obj().map(|object| {
                object
                    .keys()
                    .filter(|key| {
                        source_internal_name
                            .map(|expected| key.as_ref() == expected)
                            .unwrap_or(true)
                    })
                    .map(|key| (block_index, key.to_string()))
                    .collect::<Vec<_>>()
            })
        })
        .flatten()
        .collect();
    let [(block_index, source_key)] = candidates.as_slice() else {
        return Err(match source_internal_name {
            Some(expected) => format!(
                "Source compatibility tool VDF must contain exactly one {} entry",
                expected
            ),
            None => {
                "Source compatibility tool VDF must contain exactly one compatibility tool entry"
                    .to_string()
            }
        });
    };

    let compat_tools = compat_tool_blocks[*block_index]
        .get_mut_obj()
        .ok_or_else(|| "Source compat_tools entry is not an object".to_string())?;
    let mut tool_values = compat_tools
        .remove(source_key.as_str())
        .ok_or_else(|| "Source compatibility tool entry disappeared while rewriting".to_string())?;
    for value in &mut tool_values {
        let tool_config = value.get_mut_obj().ok_or_else(|| {
            "Source compatibility tool entry configuration is not an object".to_string()
        })?;
        tool_config.insert(
            Cow::Owned("display_name".to_string()),
            vec![Value::Str(Cow::Owned(target_display_name.to_string()))],
        );
    }
    let mut virtual_compat_tools = Obj::new();
    virtual_compat_tools.insert(Cow::Owned(target_internal_name.to_string()), tool_values);
    *compat_tool_blocks = vec![Value::Obj(virtual_compat_tools)];
    Ok(vdf)
}

pub(crate) fn compatibility_tool_registration_contract(
    vdf_path: &Path,
    expected_internal_name: Option<&str>,
) -> Result<String, String> {
    const NORMALIZED_INTERNAL_NAME: &str = "WineCellarRegistrationContract";
    const NORMALIZED_DISPLAY_NAME: &str = "Wine Cellar Registration Contract";

    let contents = fs::read_to_string(vdf_path)
        .map_err(|err| format!("Failed to read compatibility tool VDF contract: {}", err))?;
    virtualized_compatibility_tool_vdf(
        &contents,
        expected_internal_name,
        NORMALIZED_INTERNAL_NAME,
        NORMALIZED_DISPLAY_NAME,
    )
    .map(|vdf| vdf.to_string())
}

fn remove_virtual_tool_directory(base_dir: &Path, tool_dir: &Path) -> Result<(), String> {
    let canonical_base = base_dir.canonicalize().map_err(|err| {
        format!(
            "Failed to access compatibility tools base directory: {}",
            err
        )
    })?;
    let canonical_parent = tool_dir
        .parent()
        .ok_or_else(|| "Failed to resolve virtual tool parent path".to_string())?
        .canonicalize()
        .map_err(|err| format!("Failed to access virtual tool parent path: {}", err))?;
    if canonical_parent != canonical_base {
        return Err("Refusing to remove path outside compatibilitytools.d".to_string());
    }

    let target_metadata = match fs::symlink_metadata(tool_dir) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(format!("Failed to inspect virtual tool directory: {}", err)),
    };
    if target_metadata.file_type().is_symlink() {
        return fs::remove_file(tool_dir)
            .map_err(|err| format!("Error removing virtual compatibility tool symlink: {}", err));
    }
    if !target_metadata.is_dir() {
        return Err("Virtual tool path is not a directory".to_string());
    }

    let canonical_target = tool_dir
        .canonicalize()
        .map_err(|err| format!("Failed to access virtual tool directory: {}", err))?;
    if canonical_target.parent() != Some(canonical_base.as_path()) {
        return Err("Refusing to remove path outside compatibilitytools.d".to_string());
    }

    recursive_delete_dir_entry(tool_dir)
        .map_err(|err| format!("Error removing virtual compatibility tool: {}", err))
}

fn remove_virtual_tool_reserved_directories(
    base_directory: &Path,
    directory_name: &str,
) -> Result<(), String> {
    let canonical_base = base_directory.canonicalize().map_err(|err| {
        format!(
            "Failed to access compatibility tools base directory: {}",
            err
        )
    })?;
    let reserved_names = [
        format!(".wine-cellar-backup-{}", directory_name),
        format!(".wine-cellar-link-staging-{}", directory_name),
        directory_name.to_string(),
    ];
    for reserved_name in reserved_names {
        let path =
            safe_manifest_child_path(&canonical_base, &reserved_name, "virtual tool directory")?;
        remove_virtual_tool_directory(&canonical_base, &path)?;
    }
    Ok(())
}

fn validate_virtual_tool_slot_directory(
    base_directory: &Path,
    directory_name: &str,
) -> Result<PathBuf, String> {
    let canonical_base = base_directory.canonicalize().map_err(|err| {
        format!(
            "Failed to access compatibility tools base directory: {}",
            err
        )
    })?;
    let tool_dir =
        safe_manifest_child_path(&canonical_base, directory_name, "virtual tool directory")?;
    let metadata = fs::symlink_metadata(&tool_dir)
        .map_err(|err| format!("Failed to inspect virtual tool directory: {}", err))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err("Virtual tool path is not a safe directory".to_string());
    }
    let canonical_target = tool_dir
        .canonicalize()
        .map_err(|err| format!("Failed to access virtual tool directory: {}", err))?;
    if canonical_target.parent() != Some(canonical_base.as_path()) {
        return Err("Refusing to modify a virtual tool outside compatibilitytools.d".to_string());
    }
    Ok(tool_dir)
}

pub(crate) fn ensure_virtual_tool_directory_accessible(path: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path).map_err(|err| {
        format!(
            "Failed to inspect virtual tool directory permissions: {}",
            err
        )
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err("Virtual tool path is not a safe directory".to_string());
    }
    let mode = metadata.permissions().mode();
    fs::set_permissions(path, fs::Permissions::from_mode(mode | 0o055))
        .map_err(|err| format!("Failed to make virtual tool directory accessible: {}", err))
}

fn ensure_virtual_tool_vdf_accessible(path: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|err| format!("Failed to inspect virtual tool VDF permissions: {}", err))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err("Virtual tool VDF is not a safe regular file".to_string());
    }
    let mode = metadata.permissions().mode();
    fs::set_permissions(path, fs::Permissions::from_mode(mode | 0o044))
        .map_err(|err| format!("Failed to make virtual tool VDF readable: {}", err))
}

pub(crate) fn safe_manifest_child_path(
    base_directory: &Path,
    name: &str,
    description: &str,
) -> Result<PathBuf, String> {
    let mut components = Path::new(name).components();
    if !matches!(components.next(), Some(Component::Normal(_))) || components.next().is_some() {
        return Err(format!("Invalid {}", description));
    }

    let path = base_directory.join(name);
    if path.parent() != Some(base_directory) {
        return Err(format!("Invalid {}", description));
    }
    Ok(path)
}

pub(crate) fn normalize_virtual_tool_label(user_label: &str) -> Result<String, String> {
    let trimmed_label = user_label.trim().to_string();
    if trimmed_label.is_empty() {
        return Err("Virtual tool name cannot be empty".to_string());
    }

    if trimmed_label.chars().count() > MAX_VIRTUAL_TOOL_LABEL_CHARS {
        return Err(format!(
            "Virtual tool name must be {} characters or fewer",
            MAX_VIRTUAL_TOOL_LABEL_CHARS
        ));
    }

    if trimmed_label
        .chars()
        .any(|character| character.is_control())
    {
        return Err("Virtual tool name cannot contain control characters".to_string());
    }

    Ok(trimmed_label)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::steam_util::SteamUtil;
    use crate::wine_cask::app::{AppState, OperationStateSnapshot, UpdaterState};
    use crate::wine_cask::flavors::SteamClientCompatToolInfo;
    use std::collections::{HashMap, VecDeque};
    use std::os::unix::fs::{symlink, PermissionsExt};
    use std::path::Path;
    use std::sync::Arc;
    use tokio::sync::{Mutex, Notify};

    fn test_app(steam_root: &Path, manifest_path: PathBuf) -> WineCask {
        WineCask {
            steam_util: SteamUtil::new(steam_root.to_path_buf()),
            app_state: Arc::new(Mutex::new(AppState {
                catalog_flavors: Vec::new(),
                installed_tools: Vec::new(),
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
        }
    }

    fn write_registration_vdf(
        path: &Path,
        internal_name: &str,
        from_os: &str,
        to_os: &str,
        extra_field: Option<&str>,
    ) {
        let extra_field = extra_field
            .map(|value| format!("\"custom_contract\" \"{}\"", value))
            .unwrap_or_default();
        fs::write(
            path,
            format!(
                r#""compatibilitytools"
                {{
                    "compat_tools"
                    {{
                        "{}"
                        {{
                            "install_path" "."
                            "display_name" "Display Name"
                            "from_oslist" "{}"
                            "to_oslist" "{}"
                            {}
                        }}
                    }}
                }}"#,
                internal_name, from_os, to_os, extra_field
            ),
        )
        .unwrap();
    }

    fn linked_config() -> VirtualToolConfig {
        VirtualToolConfig {
            id: "virtual-1".to_string(),
            user_label: "Stable Proton".to_string(),
            steam_internal_name: "WineCellarVirtual1".to_string(),
            directory_name: "WineCellarVirtual1".to_string(),
            current_payload_release_id: Some("catalog:ProtonGE:1".to_string()),
            current_payload_name: Some("GE-Proton".to_string()),
            current_payload_flavor: Some(CompatibilityToolFlavor::ProtonGE),
            linked_source_installed_tool_id: Some("installed:GE-Proton".to_string()),
            linked_source_directory_name: Some("GE-Proton".to_string()),
            pending_payload_transaction: None,
        }
    }

    #[test]
    fn legacy_manifest_defaults_new_payload_and_link_fields() {
        let manifest: VirtualToolManifest = serde_json::from_str(
            r#"{
                "next_virtual_tool_number": 2,
                "tools": [{
                    "id": "virtual-1",
                    "user_label": "Stable Proton",
                    "steam_internal_name": "WineCellarVirtual1",
                    "directory_name": "WineCellarVirtual1",
                    "current_payload_release_id": "catalog:ProtonGE:1"
                }]
            }"#,
        )
        .unwrap();

        let tool = &manifest.tools[0];
        assert_eq!(
            tool.current_payload_release_id.as_deref(),
            Some("catalog:ProtonGE:1")
        );
        assert!(tool.current_payload_name.is_none());
        assert!(tool.current_payload_flavor.is_none());
        assert!(tool.linked_source_installed_tool_id.is_none());
        assert!(tool.linked_source_directory_name.is_none());
        assert!(tool.pending_payload_transaction.is_none());
    }

    #[test]
    fn catalog_payload_replaces_link_dependency_metadata() {
        let mut tool = linked_config();

        tool.set_catalog_payload(
            "catalog:ProtonGE:2".to_string(),
            "GE-Proton2".to_string(),
            CompatibilityToolFlavor::ProtonGE,
        );

        assert_eq!(
            tool.current_payload_release_id.as_deref(),
            Some("catalog:ProtonGE:2")
        );
        assert_eq!(tool.current_payload_name.as_deref(), Some("GE-Proton2"));
        assert!(tool.linked_source_installed_tool_id.is_none());
        assert!(tool.linked_source_directory_name.is_none());
        assert!(!tool.links_source("installed:GE-Proton", "GE-Proton"));
    }

    #[test]
    fn linked_payload_tracks_both_source_id_and_directory_identity() {
        let tool = linked_config();

        assert!(tool.links_source("installed:GE-Proton", "other"));
        assert!(tool.links_source("other", "GE-Proton"));
        assert!(!tool.links_source("other", "other"));
    }

    #[test]
    fn virtualized_vdf_preserves_selected_tool_semantics_and_drops_other_tools() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("source.vdf");
        let destination = directory.path().join("virtual.vdf");
        fs::write(
            &source,
            r#""compatibilitytools"
            {
                "root_setting" "keep-me"
                "compat_tools"
                {
                    "OtherTool"
                    {
                        "install_path" "."
                        "display_name" "Other Tool"
                        "from_oslist" "windows"
                        "to_oslist" "linux"
                    }
                    "SourceTool"
                    {
                        "install_path" "payload"
                        "display_name" "Source Tool"
                        "from_oslist" "linux"
                        "to_oslist" "windows"
                        "tool_config"
                        {
                            "nested_flag" "preserve-me"
                        }
                    }
                }
            }"#,
        )
        .unwrap();

        write_virtualized_compatibility_tool_vdf(
            &source,
            &destination,
            Some("SourceTool"),
            "WineCellarVirtual1",
            "Stable Tool",
        )
        .unwrap();

        let contents = fs::read_to_string(destination).unwrap();
        let parsed = Vdf::parse(&contents).unwrap();
        let root = parsed.value.get_obj().unwrap();
        assert_eq!(
            root.get("root_setting").unwrap()[0].get_str(),
            Some("keep-me")
        );
        let compat_tools = root.get("compat_tools").unwrap()[0].get_obj().unwrap();
        assert_eq!(compat_tools.len(), 1);
        assert!(!compat_tools.contains_key("OtherTool"));
        assert!(!compat_tools.contains_key("SourceTool"));
        let config = compat_tools.get("WineCellarVirtual1").unwrap()[0]
            .get_obj()
            .unwrap();
        assert_eq!(
            config.get("install_path").unwrap()[0].get_str(),
            Some("payload")
        );
        assert_eq!(
            config.get("display_name").unwrap()[0].get_str(),
            Some("Stable Tool")
        );
        assert_eq!(
            config.get("from_oslist").unwrap()[0].get_str(),
            Some("linux")
        );
        assert_eq!(
            config.get("to_oslist").unwrap()[0].get_str(),
            Some("windows")
        );
        let nested = config.get("tool_config").unwrap()[0].get_obj().unwrap();
        assert_eq!(
            nested.get("nested_flag").unwrap()[0].get_str(),
            Some("preserve-me")
        );
    }

    #[test]
    fn virtualized_vdf_is_readable_even_when_source_mode_is_restrictive() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("source.vdf");
        let destination = directory.path().join("virtual.vdf");
        generate_compatibility_tool_vdf(&source, "SourceTool", "Source Tool").unwrap();
        fs::set_permissions(&source, fs::Permissions::from_mode(0o600)).unwrap();

        write_virtualized_compatibility_tool_vdf(
            &source,
            &destination,
            Some("SourceTool"),
            "WineCellarVirtual1",
            "Stable Tool",
        )
        .unwrap();

        assert_eq!(
            fs::metadata(destination).unwrap().permissions().mode() & 0o044,
            0o044
        );
    }

    #[test]
    fn virtual_tool_directory_is_traversable_even_when_created_restrictively() {
        let directory = tempfile::tempdir().unwrap();
        let tool_directory = directory.path().join("WineCellarVirtual1");
        fs::create_dir(&tool_directory).unwrap();
        fs::set_permissions(&tool_directory, fs::Permissions::from_mode(0o700)).unwrap();

        ensure_virtual_tool_directory_accessible(&tool_directory).unwrap();

        assert_eq!(
            fs::metadata(tool_directory).unwrap().permissions().mode() & 0o055,
            0o055
        );
    }

    #[tokio::test]
    async fn same_registration_contract_is_allowed_without_a_visibility_snapshot() {
        let directory = tempfile::tempdir().unwrap();
        let steam_root = directory.path().join("steam");
        let base = steam_root.join("compatibilitytools.d");
        let slot = base.join("WineCellarVirtual1");
        let source = base.join("SourceTool");
        fs::create_dir_all(&slot).unwrap();
        fs::create_dir(&source).unwrap();
        write_registration_vdf(
            &slot.join("compatibilitytool.vdf"),
            "WineCellarVirtual1",
            "windows",
            "linux",
            Some("same"),
        );
        write_registration_vdf(
            &source.join("compatibilitytool.vdf"),
            "SourceTool",
            "windows",
            "linux",
            Some("same"),
        );
        let app = test_app(&steam_root, directory.path().join("virtual_tools.json"));

        app.ensure_virtual_tool_registration_compatible(
            &source.join("compatibilitytool.vdf"),
            Some("SourceTool"),
            &linked_config(),
        )
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn visible_slot_rejects_a_different_registration_contract() {
        let directory = tempfile::tempdir().unwrap();
        let steam_root = directory.path().join("steam");
        let base = steam_root.join("compatibilitytools.d");
        let slot = base.join("WineCellarVirtual1");
        let source = base.join("SourceTool");
        fs::create_dir_all(&slot).unwrap();
        fs::create_dir(&source).unwrap();
        write_registration_vdf(
            &slot.join("compatibilitytool.vdf"),
            "WineCellarVirtual1",
            "windows",
            "linux",
            None,
        );
        write_registration_vdf(
            &source.join("compatibilitytool.vdf"),
            "SourceTool",
            "linux",
            "windows",
            Some("different"),
        );
        let app = test_app(&steam_root, directory.path().join("virtual_tools.json"));
        app.app_state.lock().await.steam_visible_tools = vec![SteamClientCompatToolInfo {
            str_tool_name: "WineCellarVirtual1".to_string(),
            str_display_name: "Stable Proton".to_string(),
        }];

        let error = app
            .ensure_virtual_tool_registration_compatible(
                &source.join("compatibilitytool.vdf"),
                Some("SourceTool"),
                &linked_config(),
            )
            .await
            .unwrap_err();

        assert!(error.contains("different Steam registration"));
    }

    #[tokio::test]
    async fn definitely_unseen_slot_allows_a_different_registration_contract() {
        let directory = tempfile::tempdir().unwrap();
        let steam_root = directory.path().join("steam");
        let base = steam_root.join("compatibilitytools.d");
        let slot = base.join("WineCellarVirtual1");
        let source = base.join("SourceTool");
        fs::create_dir_all(&slot).unwrap();
        fs::create_dir(&source).unwrap();
        write_registration_vdf(
            &slot.join("compatibilitytool.vdf"),
            "WineCellarVirtual1",
            "windows",
            "linux",
            None,
        );
        write_registration_vdf(
            &source.join("compatibilitytool.vdf"),
            "SourceTool",
            "linux",
            "windows",
            Some("different"),
        );
        let app = test_app(&steam_root, directory.path().join("virtual_tools.json"));
        app.app_state.lock().await.steam_visible_tools = vec![SteamClientCompatToolInfo {
            str_tool_name: "SomeOtherTool".to_string(),
            str_display_name: "Other Tool".to_string(),
        }];

        app.ensure_virtual_tool_registration_compatible(
            &source.join("compatibilitytool.vdf"),
            Some("SourceTool"),
            &linked_config(),
        )
        .await
        .unwrap();
    }

    #[test]
    fn create_fails_closed_when_manifest_is_malformed() {
        let directory = tempfile::tempdir().unwrap();
        let steam_root = directory.path().join("steam");
        let compatibility_tools = steam_root.join("compatibilitytools.d");
        let occupied = compatibility_tools.join("WineCellarVirtual1");
        fs::create_dir_all(&occupied).unwrap();
        fs::write(occupied.join("keep"), "original").unwrap();
        let manifest_path = directory.path().join("virtual_tools.json");
        fs::write(&manifest_path, "{ malformed").unwrap();
        let app = test_app(&steam_root, manifest_path);

        let error = app
            .create_virtual_tool_slot("Stable Tool".to_string())
            .unwrap_err();

        assert!(error.contains("Failed to parse virtual tool manifest"));
        assert_eq!(
            fs::read_to_string(occupied.join("keep")).unwrap(),
            "original"
        );
        assert!(!occupied.join("compatibilitytool.vdf").exists());
    }

    #[test]
    fn create_skips_an_occupied_orphan_identity() {
        let directory = tempfile::tempdir().unwrap();
        let steam_root = directory.path().join("steam");
        let compatibility_tools = steam_root.join("compatibilitytools.d");
        let occupied = compatibility_tools.join("WineCellarVirtual1");
        fs::create_dir_all(&occupied).unwrap();
        fs::write(occupied.join("keep"), "original").unwrap();
        let manifest_path = directory.path().join("virtual_tools.json");
        let app = test_app(&steam_root, manifest_path);

        app.create_virtual_tool_slot("Stable Tool".to_string())
            .unwrap();

        let manifest = app.try_load_virtual_tool_manifest().unwrap();
        assert_eq!(manifest.tools.len(), 1);
        assert_eq!(manifest.tools[0].directory_name, "WineCellarVirtual2");
        assert_eq!(manifest.next_virtual_tool_number, 3);
        assert_eq!(
            fs::read_to_string(occupied.join("keep")).unwrap(),
            "original"
        );
        assert!(compatibility_tools
            .join("WineCellarVirtual2/compatibilitytool.vdf")
            .is_file());
    }

    #[test]
    fn removing_a_root_symlink_unlinks_only_the_slot() {
        let directory = tempfile::tempdir().unwrap();
        let base = directory.path().join("compatibilitytools.d");
        let source = base.join("SourceTool");
        let slot = base.join("WineCellarVirtual1");
        fs::create_dir_all(&source).unwrap();
        fs::write(source.join("keep"), "source payload").unwrap();
        symlink("SourceTool", &slot).unwrap();

        remove_virtual_tool_directory(&base, &slot).unwrap();

        assert!(fs::symlink_metadata(&slot).is_err());
        assert_eq!(
            fs::read_to_string(source.join("keep")).unwrap(),
            "source payload"
        );
    }

    #[test]
    fn removing_a_slot_clears_reserved_backup_and_staging_directories() {
        let directory = tempfile::tempdir().unwrap();
        let base = directory.path().join("compatibilitytools.d");
        let slot = base.join("WineCellarVirtual1");
        let backup = base.join(".wine-cellar-backup-WineCellarVirtual1");
        let staging = base.join(".wine-cellar-link-staging-WineCellarVirtual1");
        for path in [&slot, &backup, &staging] {
            fs::create_dir_all(path).unwrap();
            fs::write(path.join("payload"), "data").unwrap();
        }

        remove_virtual_tool_reserved_directories(&base, "WineCellarVirtual1").unwrap();

        assert!(fs::symlink_metadata(slot).is_err());
        assert!(fs::symlink_metadata(backup).is_err());
        assert!(fs::symlink_metadata(staging).is_err());
    }

    #[test]
    fn rename_rejects_a_root_symlink_without_rewriting_its_source() {
        let directory = tempfile::tempdir().unwrap();
        let steam_root = directory.path().join("steam");
        let base = steam_root.join("compatibilitytools.d");
        let source = base.join("SourceTool");
        let slot = base.join("WineCellarVirtual1");
        fs::create_dir_all(&source).unwrap();
        generate_compatibility_tool_vdf(
            source.join("compatibilitytool.vdf"),
            "WineCellarVirtual1",
            "Original Label",
        )
        .unwrap();
        symlink("SourceTool", &slot).unwrap();
        let manifest_path = directory.path().join("virtual_tools.json");
        let app = test_app(&steam_root, manifest_path);
        let mut config = linked_config();
        config.user_label = "Original Label".to_string();
        app.save_virtual_tool_manifest(&VirtualToolManifest {
            next_virtual_tool_number: 2,
            tools: vec![config],
        })
        .unwrap();
        let original_vdf = fs::read_to_string(source.join("compatibilitytool.vdf")).unwrap();

        let error = app
            .rename_virtual_tool_slot("virtual-1", "New Label".to_string())
            .unwrap_err();

        assert!(error.contains("not a safe directory"));
        assert_eq!(
            fs::read_to_string(source.join("compatibilitytool.vdf")).unwrap(),
            original_vdf
        );
        assert_eq!(
            app.try_load_virtual_tool_manifest().unwrap().tools[0].user_label,
            "Original Label"
        );
    }

    #[test]
    fn dependency_lookup_fails_closed_for_a_malformed_manifest() {
        let directory = tempfile::tempdir().unwrap();
        let steam_root = directory.path().join("steam");
        fs::create_dir_all(steam_root.join("compatibilitytools.d")).unwrap();
        let manifest_path = directory.path().join("virtual_tools.json");
        fs::write(&manifest_path, "{ malformed").unwrap();
        let app = WineCask {
            steam_util: SteamUtil::new(steam_root),
            app_state: Arc::new(Mutex::new(AppState {
                catalog_flavors: Vec::new(),
                installed_tools: Vec::new(),
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

        let error = app
            .virtual_tools_linking_source("installed:GE-Proton", "GE-Proton")
            .unwrap_err();
        assert!(error.contains("Failed to parse virtual tool manifest"));
    }
}

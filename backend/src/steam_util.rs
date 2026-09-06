use std::collections::HashMap;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::fs;
use std::path::PathBuf;
use std::{env, fmt};

use keyvalues_parser::Vdf;
use log::{error, info, warn};
use serde::Serialize;

#[derive(Debug, Clone)]
pub enum SteamUtilError {
    HomeDirectoryNotFound,
    SteamDirectoryNotFound,
    SteamAppsDirectoryNotFound,
    LibraryFoldersVdfNotFound,
    SteamConfigVdfNotFound,
    VdfParsingError(String),
    VdfMissingEntry(String),
}

#[derive(Clone)]
pub struct SteamUtil {
    steam_path: PathBuf,
}

#[derive(Serialize, Clone)]
pub struct CompatibilityTool {
    pub path: PathBuf,
    pub directory_name: String,
    pub internal_name: String,
    pub display_name: String,
    pub from_os_list: String,
    pub to_os_list: String,
}

#[derive(Serialize)]
pub struct SteamApp {
    pub app_id: u64,
    pub name: String,
}

impl SteamUtil {
    pub fn new(steam_home: PathBuf) -> Self {
        Self {
            steam_path: steam_home,
        }
    }

    pub fn find_steam_directory(
        user_home_directory: Option<String>,
    ) -> Result<PathBuf, SteamUtilError> {
        let possible_steam_roots = [
            ".local/share/Steam",
            ".steam/root",
            ".steam/steam",
            ".steam/debian-installation",
            ".var/app/com.valvesoftware.Steam/data/Steam",
        ];

        let user_profile = user_home_directory.map(PathBuf::from).or_else(|| {
            env::var_os("USERPROFILE")
                .map(PathBuf::from)
                .or_else(|| env::var_os("HOME").map(PathBuf::from))
        });

        if let Some(user_profile) = user_profile {
            info!("Looking for Steam directory in {}", user_profile.display());
            for steam_dir in &possible_steam_roots {
                let expanded_steam_dir = user_profile.join(steam_dir);
                let config_vdf = expanded_steam_dir.join("config").join("config.vdf");
                let libraryfolders_vdf = expanded_steam_dir
                    .join("steamapps")
                    .join("libraryfolders.vdf");

                if config_vdf.exists() && libraryfolders_vdf.exists() {
                    info!("Found Steam directory: {}", expanded_steam_dir.display());
                    return Ok(expanded_steam_dir);
                }
            }
        } else {
            return Err(SteamUtilError::HomeDirectoryNotFound);
        }

        Err(SteamUtilError::SteamDirectoryNotFound)
    }

    pub fn get_steam_compatibility_tools_directory(&self) -> PathBuf {
        let path = self.steam_path.join("compatibilitytools.d");
        if !path.exists() && self.steam_path.exists() {
            warn!("Steam compatibility tools directory does not exist, creating it...");
            if fs::create_dir(&path).is_err() {
                error!(
                    "Failed to create compatibility tools directory at {}",
                    path.display()
                );
            }
        }
        path
    }

    pub fn read_compatibility_tool_from_vdf_path(
        &self,
        compat_tool_vdf: &PathBuf,
    ) -> Result<CompatibilityTool, SteamUtilError> {
        let vdf_text = fs::read_to_string(compat_tool_vdf)
            .map_err(|err| SteamUtilError::VdfParsingError(err.to_string()))?;

        let vdf = Vdf::parse(&vdf_text)
            .map_err(|err| SteamUtilError::VdfParsingError(err.to_string()))?;

        let compat_tool_obj = vdf
            .value
            .get_obj()
            .and_then(|f| f.values().next())
            .and_then(|f| f.first())
            .and_then(|f| f.get_obj())
            .ok_or_else(|| SteamUtilError::VdfParsingError("Invalid VDF structure".to_string()))?;

        let path = compat_tool_vdf
            .parent()
            .ok_or_else(|| {
                SteamUtilError::VdfMissingEntry("Parent directory not found".to_string())
            })?
            .to_path_buf();

        let directory_name = path
            .file_name()
            .and_then(|o| o.to_str())
            .ok_or_else(|| SteamUtilError::VdfMissingEntry("Directory name not found".to_string()))?
            .to_string();

        let internal_name = compat_tool_obj
            .keys()
            .next()
            .ok_or_else(|| SteamUtilError::VdfMissingEntry("Internal name not found".to_string()))?
            .to_string();

        let internal_value = compat_tool_obj
            .values()
            .next()
            .and_then(|o| o.first())
            .and_then(|o| o.get_obj())
            .ok_or_else(|| {
                SteamUtilError::VdfMissingEntry("Internal value not found".to_string())
            })?;

        let display_name = internal_value
            .get("display_name")
            .and_then(|o| o.first())
            .and_then(|o| o.get_str())
            .map(|o| o.to_string())
            .ok_or_else(|| SteamUtilError::VdfMissingEntry("Display name not found".to_string()))?;

        let from_os_list = internal_value
            .get("from_oslist")
            .and_then(|o| o.first())
            .and_then(|o| o.get_str())
            .map(|o| o.to_string())
            .ok_or_else(|| SteamUtilError::VdfMissingEntry("From OS list not found".to_string()))?;

        let to_os_list = internal_value
            .get("to_oslist")
            .and_then(|o| o.first())
            .and_then(|o| o.get_str())
            .map(|o| o.to_string())
            .ok_or_else(|| SteamUtilError::VdfMissingEntry("To OS list not found".to_string()))?;

        Ok(CompatibilityTool {
            path,
            directory_name,
            internal_name,
            display_name,
            from_os_list,
            to_os_list,
        })
    }

    pub fn list_compatibility_tools(&self) -> Result<Vec<CompatibilityTool>, SteamUtilError> {
        let compatibility_tools_directory = self.get_steam_compatibility_tools_directory();

        let entries = fs::read_dir(compatibility_tools_directory)
            .map_err(|_| SteamUtilError::SteamAppsDirectoryNotFound)?;

        let compat_tools: Vec<CompatibilityTool> = entries
            .filter_map(Result::ok)
            .filter(|x| {
                let is_wine_cellar_internal_dir = x
                    .file_name()
                    .to_str()
                    .map(|name| name.starts_with(".wine-cellar-"))
                    .unwrap_or(false);
                x.metadata().map(|m| m.is_dir()).unwrap_or(false)
                    && !is_wine_cellar_internal_dir
                    && x.path().join("compatibilitytool.vdf").exists()
            })
            .flat_map(|x| {
                self.read_compatibility_tool_from_vdf_path(&x.path().join("compatibilitytool.vdf"))
                    .map_err(|err| {
                        error!("Error reading compatibility tool vdf: {}", err);
                        err
                    })
            })
            .collect();

        Ok(compat_tools)
    }

    pub fn get_compatibility_tools_mappings(&self) -> Result<HashMap<u64, String>, SteamUtilError> {
        let steam_config_file = self.steam_path.join("config").join("config.vdf");

        if !steam_config_file.exists() {
            return Err(SteamUtilError::SteamConfigVdfNotFound);
        }

        let config = fs::read_to_string(&steam_config_file)
            .map_err(|_| SteamUtilError::SteamConfigVdfNotFound)?;

        let config_vdf = Vdf::parse(&config).map_err(|_| {
            SteamUtilError::VdfParsingError(steam_config_file.to_string_lossy().to_string())
        })?;

        let software_vdf_obj = config_vdf
            .value
            .get_obj()
            .and_then(|config| config.get("Software"))
            .and_then(|o| o.first())
            .and_then(|f| f.get_obj())
            .ok_or_else(|| {
                SteamUtilError::VdfMissingEntry("Software object not found".to_string())
            })?;

        let valve_vdf_obj = software_vdf_obj
            .get("Valve")
            .or(software_vdf_obj.get("valve"))
            .and_then(|valve_obj| valve_obj.first())
            .and_then(|o| o.get_obj())
            .ok_or_else(|| SteamUtilError::VdfMissingEntry("Valve object not found".to_string()))?;

        let steam_obj = valve_vdf_obj
            .get("Steam")
            .and_then(|steam| steam.first())
            .and_then(|o| o.get_obj())
            .ok_or_else(|| SteamUtilError::VdfMissingEntry("Steam object not found".to_string()))?;

        let compat_tool_mapping = steam_obj
            .get("CompatToolMapping")
            .and_then(|o| o.first())
            .and_then(|f| f.get_obj())
            .ok_or_else(|| {
                SteamUtilError::VdfMissingEntry("CompatToolMapping object not found".to_string())
            })?;

        let mut compatibility_tools_mappings: HashMap<u64, String> = HashMap::new();
        for (key, value) in compat_tool_mapping.iter() {
            let key: u64 = key.parse().map_err(|_| {
                SteamUtilError::VdfMissingEntry("Error parsing key to u64".to_string())
            })?;
            let key_obj = value.first().and_then(|o| o.get_obj()).ok_or_else(|| {
                SteamUtilError::VdfMissingEntry("Key object not found".to_string())
            })?;
            let compat_tool_name = key_obj
                .get("name")
                .and_then(|n| n.first())
                .and_then(|o| o.get_str())
                .ok_or_else(|| {
                    SteamUtilError::VdfMissingEntry(
                        "Compat tool name not found or invalid".to_string(),
                    )
                })?
                .to_string();
            if !compat_tool_name.is_empty() {
                compatibility_tools_mappings.insert(key, compat_tool_name);
            }
        }

        Ok(compatibility_tools_mappings)
    }

    /// Returns the compatibility tools that users explicitly forced for individual apps.
    ///
    /// Steam also stores automatic compatibility selections in `CompatToolMapping`. Those
    /// entries currently use a lower priority, while the compatibility-properties UI writes
    /// user-forced selections with priority 250. Keep those two concepts separate so callers do
    /// not present an automatic Steam choice as a user override.
    pub fn get_forced_compatibility_tools_mappings(
        &self,
    ) -> Result<HashMap<u64, String>, SteamUtilError> {
        const USER_FORCED_PRIORITY: u32 = 250;

        let steam_config_file = self.steam_path.join("config").join("config.vdf");

        if !steam_config_file.exists() {
            return Err(SteamUtilError::SteamConfigVdfNotFound);
        }

        let config = fs::read_to_string(&steam_config_file)
            .map_err(|_| SteamUtilError::SteamConfigVdfNotFound)?;

        let config_vdf = Vdf::parse(&config).map_err(|_| {
            SteamUtilError::VdfParsingError(steam_config_file.to_string_lossy().to_string())
        })?;

        let software_vdf_obj = config_vdf
            .value
            .get_obj()
            .and_then(|config| config.get("Software"))
            .and_then(|o| o.first())
            .and_then(|f| f.get_obj())
            .ok_or_else(|| {
                SteamUtilError::VdfMissingEntry("Software object not found".to_string())
            })?;

        let valve_vdf_obj = software_vdf_obj
            .get("Valve")
            .or(software_vdf_obj.get("valve"))
            .and_then(|valve_obj| valve_obj.first())
            .and_then(|o| o.get_obj())
            .ok_or_else(|| SteamUtilError::VdfMissingEntry("Valve object not found".to_string()))?;

        let steam_obj = valve_vdf_obj
            .get("Steam")
            .and_then(|steam| steam.first())
            .and_then(|o| o.get_obj())
            .ok_or_else(|| SteamUtilError::VdfMissingEntry("Steam object not found".to_string()))?;

        let Some(compat_tool_mapping_values) = steam_obj.get("CompatToolMapping") else {
            return Ok(HashMap::new());
        };
        let compat_tool_mapping = compat_tool_mapping_values
            .first()
            .and_then(|value| value.get_obj())
            .ok_or_else(|| {
                SteamUtilError::VdfMissingEntry("CompatToolMapping object is malformed".to_string())
            })?;

        let mut forced_mappings = HashMap::new();
        for (raw_app_id, values) in compat_tool_mapping.iter() {
            let app_id = match raw_app_id.parse::<u64>() {
                Ok(app_id) => app_id,
                Err(_) => {
                    warn!(
                        "Skipping malformed Steam compatibility mapping app ID: {}",
                        raw_app_id
                    );
                    continue;
                }
            };

            // App ID 0 represents Steam's global default rather than an application override.
            if app_id == 0 {
                continue;
            }

            let Some(mapping) = values.first().and_then(|value| value.get_obj()) else {
                warn!(
                    "Skipping malformed Steam compatibility mapping for app ID {}",
                    app_id
                );
                continue;
            };

            let Some(name) = mapping
                .get("name")
                .and_then(|values| values.first())
                .and_then(|value| value.get_str())
            else {
                warn!(
                    "Skipping Steam compatibility mapping with no tool name for app ID {}",
                    app_id
                );
                continue;
            };
            if name.is_empty() {
                continue;
            }

            let Some(raw_priority) = mapping
                .get("priority")
                .and_then(|values| values.first())
                .and_then(|value| value.get_str())
            else {
                warn!(
                    "Skipping Steam compatibility mapping with no priority for app ID {}",
                    app_id
                );
                continue;
            };

            let priority = match raw_priority.parse::<u32>() {
                Ok(priority) => priority,
                Err(_) => {
                    warn!(
                        "Skipping Steam compatibility mapping with invalid priority for app ID {}",
                        app_id
                    );
                    continue;
                }
            };

            if priority == USER_FORCED_PRIORITY {
                forced_mappings.insert(app_id, name.to_string());
            }
        }

        Ok(forced_mappings)
    }

    pub fn list_library_folders(&self) -> Result<Vec<PathBuf>, SteamUtilError> {
        let steam_apps_directory = self.steam_path.join("steamapps");

        if !steam_apps_directory.exists() {
            return Err(SteamUtilError::SteamAppsDirectoryNotFound);
        }

        let library_folders_vdf_file = steam_apps_directory.join("libraryfolders.vdf");

        if !library_folders_vdf_file.exists() {
            return Err(SteamUtilError::LibraryFoldersVdfNotFound);
        }

        let library_folders_vdf = fs::read_to_string(&library_folders_vdf_file)
            .map_err(|err| SteamUtilError::VdfParsingError(err.to_string()))?;
        let vdf = Vdf::parse(&library_folders_vdf)
            .map_err(|err| SteamUtilError::VdfParsingError(err.to_string()))?;
        let app_state_obj = vdf.value.get_obj().ok_or_else(|| {
            SteamUtilError::VdfMissingEntry("Invalid library folders VDF".to_string())
        })?;

        let mut library_folders: Vec<PathBuf> = Vec::new();

        for value in app_state_obj.values() {
            let Some(key_obj) = value.first().and_then(|o| o.get_obj()) else {
                continue;
            };
            let Some(path) = key_obj
                .get("path")
                .and_then(|o| o.first())
                .and_then(|o| o.get_str())
                .map(|path| path.to_string())
            else {
                continue;
            };
            if !path.is_empty() {
                library_folders.push(PathBuf::from(path));
            }
        }

        Ok(library_folders)
    }

    pub fn list_installed_games(&self) -> Result<Vec<SteamApp>, SteamUtilError> {
        let mut apps: Vec<SteamApp> = Vec::new();
        match self.list_library_folders() {
            Ok(library_folders) => {
                for library_folder in library_folders {
                    let library_folder = library_folder.join("steamapps");
                    if !library_folder.exists() {
                        error!(
                            "Library folder {} does not exist",
                            library_folder.to_string_lossy()
                        );
                        continue;
                    }
                    match &mut self.find_installed_games(library_folder.clone()) {
                        Ok(steam_apps) => apps.append(steam_apps),
                        Err(err) => {
                            error!(
                                "Failed to find installed games in library folder {}: {}",
                                &library_folder.to_string_lossy(),
                                err
                            );
                            return Err(err.clone());
                        }
                    }
                }
            }
            Err(err) => {
                error!("Failed to list library folders: {}", err);
                return Err(err);
            }
        }
        Ok(apps)
    }

    pub fn find_installed_games(
        &self,
        steam_apps_directory: PathBuf,
    ) -> Result<Vec<SteamApp>, SteamUtilError> {
        let entries = fs::read_dir(steam_apps_directory)
            .map_err(|_err| SteamUtilError::SteamAppsDirectoryNotFound)?;

        let apps: Vec<SteamApp> = entries
            .filter_map(Result::ok)
            .filter(|x| x.path().extension().unwrap_or_default().eq("acf"))
            .flat_map(|file| {
                Self::read_app_manifest_to_steam_app(file.path()).map_err(|err| {
                    error!("Error reading app manifest: {}", err);
                    err
                })
            })
            .collect();

        Ok(apps)
    }

    pub fn read_app_manifest_to_steam_app(path_buf: PathBuf) -> Result<SteamApp, SteamUtilError> {
        let app_manifest = fs::read_to_string(path_buf)
            .map_err(|err| SteamUtilError::VdfParsingError(err.to_string()))?;
        let vdf = Vdf::parse(&app_manifest)
            .map_err(|err| SteamUtilError::VdfParsingError(err.to_string()))?;
        let app_id: u64 = vdf
            .value
            .get_obj()
            .and_then(|f| f.get("appid"))
            .and_then(|f| f.first())
            .and_then(|f| f.get_str())
            .and_then(|f| f.parse::<u64>().ok())
            .ok_or_else(|| SteamUtilError::VdfMissingEntry("appid".to_string()))?;
        let name: String = vdf
            .value
            .get_obj()
            .and_then(|f| f.get("name"))
            .and_then(|f| f.first())
            .and_then(|f| f.get_str())
            .map(|f| f.to_string())
            .ok_or_else(|| SteamUtilError::VdfMissingEntry("name".to_string()))?;
        Ok(SteamApp { app_id, name })
    }
}

impl Display for SteamUtilError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            SteamUtilError::HomeDirectoryNotFound => write!(f, "Home directory not found"),
            SteamUtilError::SteamDirectoryNotFound => write!(f, "Steam directory not found"),
            SteamUtilError::SteamAppsDirectoryNotFound => {
                write!(f, "Steam apps directory not found")
            }
            SteamUtilError::LibraryFoldersVdfNotFound => {
                write!(f, "Steam library folders VDF file not found")
            }
            SteamUtilError::SteamConfigVdfNotFound => write!(f, "Steam config file not found"),
            SteamUtilError::VdfParsingError(msg) => write!(f, "Failed to parse VDF file: {}", msg),
            SteamUtilError::VdfMissingEntry(msg) => write!(f, "Missing VDF entry: {}", msg),
        }
    }
}

impl Error for SteamUtilError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::{tempdir, TempDir};

    fn create_test_steam_directory() -> TempDir {
        let steam_dir = tempdir().expect("Failed to create temporary directory");
        let root_dir = steam_dir.path().join("root");
        let compatibility_tools_dir = root_dir.join("compatibilitytools.d");
        let config_dir = root_dir.join("config");
        let config_file = config_dir.join("config.vdf");
        let steamapps_dir = root_dir.join("steamapps");

        fs::create_dir_all(&compatibility_tools_dir)
            .expect("Failed to create compatibility tools directory");
        fs::create_dir_all(&config_dir).expect("Failed to create config directory");
        fs::create_dir_all(&steamapps_dir).expect("Failed to create steamapps directory");

        let compat_tool_1_dir = compatibility_tools_dir.join("compat_tool_1");
        fs::create_dir_all(&compat_tool_1_dir)
            .expect("Failed to create compatibility tool directory");
        let compat_tool_1_vdf = compat_tool_1_dir.join("compatibilitytool.vdf");
        fs::write(
            compat_tool_1_vdf,
            r#""compatibilitytools"
            {
              "compat_tools"
              {
                "Sample-Compatibility-Tool-1"
                {
                  "install_path" "."
                  "display_name" "Sample Compatibility Tool 1"
                  "from_oslist"  "windows"
                  "to_oslist"    "linux"
                }
              }
            }"#,
        )
        .expect("Failed to write compatibility tool VDF file");

        let compat_tool_2_dir = compatibility_tools_dir.join("compat_tool_2");
        fs::create_dir_all(&compat_tool_2_dir)
            .expect("Failed to create compatibility tool directory");
        let compat_tool_2_vdf = compat_tool_2_dir.join("compatibilitytool.vdf");
        fs::write(
            compat_tool_2_vdf,
            r#""compatibilitytools"
            {
              "compat_tools"
              {
                "Sample-Compatibility-Tool-2"
                {
                  "install_path" "."
                  "display_name" "Sample Compatibility Tool 2"
                  "from_oslist"  "windows"
                  "to_oslist"    "linux"
                }
              }
            }"#,
        )
        .expect("Failed to write compatibility tool VDF file");

        fs::write(
            config_file,
            r#""InstallConfigStore"
            {
                "Software"
                {
                    "Valve"
                    {
                        "Steam"
                        {
                            "CompatToolMapping"
                            {
                                "123456"
                                {
                                    "name" "Sample Compatibility Tool 1"
                                }
                                "654321"
                                {
                                    "name" "Sample Compatibility Tool 2"
                                }
                            }
                        }
                    }
                }
            }"#,
        )
        .expect("Failed to write config file");

        fs::write(
            steamapps_dir.join("libraryfolders.vdf"),
            r#""libraryfolders"
            {
                "0"
                {
                    "path" "TEST_PATH"
                }
            }"#,
        )
        .expect("Failed to write library folders vdf");

        let app_manifest = steamapps_dir.join("appmanifest_123456.acf");
        fs::write(
            app_manifest,
            r#""AppState"
            {
                "appid" "123456"
                "name" "Sample Game"
            }"#,
        )
        .expect("Failed to write app manifest");

        steam_dir
    }

    #[test]
    fn test_list_compatibility_tools() {
        let steam_dir = create_test_steam_directory();
        let steam_util = SteamUtil::new(steam_dir.path().join("root"));

        let mut compat_tools = steam_util
            .list_compatibility_tools()
            .expect("Failed to list compatibility tools");

        compat_tools.sort_by(|a, b| a.display_name.cmp(&b.display_name));

        assert_eq!(compat_tools.len(), 2);
        assert_eq!(compat_tools[0].display_name, "Sample Compatibility Tool 1");
        assert_eq!(compat_tools[1].display_name, "Sample Compatibility Tool 2");
    }

    #[test]
    fn test_get_compatibility_tools_mappings() {
        let steam_dir = create_test_steam_directory();
        let steam_util = SteamUtil::new(steam_dir.path().join("root"));

        let mappings = steam_util
            .get_compatibility_tools_mappings()
            .expect("Failed to get compatibility tools mappings");

        assert_eq!(mappings.len(), 2);
        assert_eq!(
            mappings
                .get(&123456)
                .expect("Expected mapping for app ID 123456 not found"),
            "Sample Compatibility Tool 1"
        );
        assert_eq!(
            mappings
                .get(&654321)
                .expect("Expected mapping for app ID 654321 not found"),
            "Sample Compatibility Tool 2"
        );
    }

    #[test]
    fn test_get_forced_compatibility_tools_mappings_only_returns_user_overrides() {
        let steam_dir = create_test_steam_directory();
        let steam_root = steam_dir.path().join("root");
        fs::write(
            steam_root.join("config").join("config.vdf"),
            r#""InstallConfigStore"
            {
                "Software"
                {
                    "Valve"
                    {
                        "Steam"
                        {
                            "CompatToolMapping"
                            {
                                "0"
                                {
                                    "name" "global-default"
                                    "priority" "250"
                                }
                                "101"
                                {
                                    "name" "user-forced-tool"
                                    "priority" "250"
                                }
                                "102"
                                {
                                    "name" "automatic-tool"
                                    "priority" "100"
                                }
                                "not-an-app-id"
                                {
                                    "name" "ignored-tool"
                                    "priority" "250"
                                }
                                "103" "not-an-object"
                                "104"
                                {
                                    "priority" "250"
                                }
                                "105"
                                {
                                    "name" "missing-priority"
                                }
                                "106"
                                {
                                    "name" "invalid-priority"
                                    "priority" "high"
                                }
                            }
                        }
                    }
                }
            }"#,
        )
        .expect("Failed to write config file");

        let mappings = SteamUtil::new(steam_root)
            .get_forced_compatibility_tools_mappings()
            .expect("Failed to get forced compatibility tool mappings");

        assert_eq!(mappings.len(), 1);
        assert_eq!(mappings.get(&101), Some(&"user-forced-tool".to_string()));
    }

    #[test]
    fn test_get_forced_compatibility_tools_mappings_allows_absent_mapping() {
        let steam_dir = create_test_steam_directory();
        let steam_root = steam_dir.path().join("root");
        fs::write(
            steam_root.join("config").join("config.vdf"),
            r#""InstallConfigStore"
            {
                "Software"
                {
                    "Valve"
                    {
                        "Steam"
                        {
                            "SomeOtherSetting" "1"
                        }
                    }
                }
            }"#,
        )
        .expect("Failed to write config file");

        let mappings = SteamUtil::new(steam_root)
            .get_forced_compatibility_tools_mappings()
            .expect("Absent compatibility mappings should be valid");

        assert!(mappings.is_empty());
    }

    #[test]
    fn test_list_installed_games() {
        let steam_dir = create_test_steam_directory();
        let steam_util = SteamUtil::new(steam_dir.path().join("root"));

        let games = steam_util
            .find_installed_games(steam_dir.path().join("root").join("steamapps"))
            .expect("Failed to list installed games");

        assert_eq!(games.len(), 1);
        assert_eq!(games[0].app_id, 123456);
        assert_eq!(games[0].name, "Sample Game");
    }

    #[test]
    fn test_list_library_folders_ignores_metadata_entries() {
        let steam_dir = create_test_steam_directory();
        let steam_root = steam_dir.path().join("root");
        fs::write(
            steam_root.join("steamapps").join("libraryfolders.vdf"),
            format!(
                r#""libraryfolders"
            {{
                "contentstatsid" "1234567890"
                "0"
                {{
                    "path" "{}"
                }}
            }}"#,
                steam_root.display()
            ),
        )
        .expect("Failed to write library folders vdf");

        let steam_util = SteamUtil::new(steam_root.clone());
        let library_folders = steam_util
            .list_library_folders()
            .expect("Failed to list library folders");

        assert_eq!(library_folders, vec![steam_root]);
    }
}

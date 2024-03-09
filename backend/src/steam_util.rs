use std::collections::HashMap;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::fs;
use std::path::PathBuf;
use std::{env, fmt};

use keyvalues_parser::Vdf;
use log::{error, info, warn};
use serde::Serialize;

/// Represents errors that can occur while using `SteamUtil`.
#[derive(Debug, Clone)]
pub enum SteamUtilError {
    /// The home directory could not be found.
    HomeDirectoryNotFound,
    /// The steam directory could not be found.
    SteamDirectoryNotFound,
    /// The compatibility tools directory could not be created.
    CompatibilityToolsDirectoryCreationFailed,
    /// The steam applications directory could not be found.
    SteamAppsDirectoryNotFound,
    /// The library folders vdf could not be found.
    LibraryFoldersVdfNotFound,
    /// The Steam configuration vdf could not be found.
    SteamConfigVdfNotFound,
    /// Vdf parsing error, that returns a string with the error.
    VdfParsingError(String),
    /// Missing Vdf Entry
    VdfMissingEntry(String),
}

/// Utility for working with Steam directories and settings.
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
    /// Creates a new instance of `SteamUtil` with the given Steam home directory.
    pub fn new(steam_home: PathBuf) -> Self {
        Self {
            steam_path: steam_home,
        }
    }

    /// Finds the Steam directory.
    pub fn find_steam_directory(
        user_home_directory: Option<String>,
    ) -> Result<PathBuf, SteamUtilError> {
        // Possible Steam root directories
        let possible_steam_roots = [
            // todo: handle multiple installations perhaps a dropdown in frontend if we detect multiple installation
            ".local/share/Steam",
            ".steam/root",
            ".steam/steam",
            ".steam/debian-installation",
            ".var/app/com.valvesoftware.Steam/data/Steam", // flatpak
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
                let ct_dir = expanded_steam_dir.join("config");
                let config_vdf = ct_dir.join("config.vdf"); // this does exist on clean install
                let libraryfolders_vdf = ct_dir.join("libraryfolders.vdf"); // On a clean install doesn't exist, it's generated after login

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

    pub fn find() -> Result<Self, SteamUtilError> {
        match SteamUtil::find_steam_directory(None) {
            Ok(steam_home) => Ok(Self {
                steam_path: steam_home,
            }),
            Err(err) => Err(err),
        }
    }

    pub fn get_steam_compatibility_tools_directory(&self) -> PathBuf {
        let path = self.steam_path.join("compatibilitytools.d"); // Apparently this is not created by default
        if !path.exists() && self.steam_path.exists() {
            warn!("Steam compatibility tools directory does not exist, creating it...");
            fs::create_dir(&path)
                .map_err(|_err| SteamUtilError::CompatibilityToolsDirectoryCreationFailed)
                .unwrap();
        }
        path
    }

    pub fn read_compatibility_tool_from_vdf_path(
        &self,
        compat_tool_vdf: &PathBuf,
    ) -> Result<CompatibilityTool, SteamUtilError> {
        // Read the content of the VDF file
        let vdf_text = fs::read_to_string(compat_tool_vdf)
            .map_err(|err| SteamUtilError::VdfParsingError(err.to_string()))?;

        // Parse the VDF text into a Vdf struct
        let vdf = Vdf::parse(&vdf_text)
            .map_err(|err| SteamUtilError::VdfParsingError(err.to_string()))?;

        // Extract the compatibility tool object from the parsed VDF
        let compat_tool_obj = vdf
            .value
            .get_obj()
            .and_then(|f| f.values().next())
            .and_then(|f| f.first())
            .and_then(|f| f.get_obj())
            .ok_or_else(|| SteamUtilError::VdfParsingError("Invalid VDF structure".to_string()))?;

        // Extract the path from the compatibility tool VDF file
        let path = compat_tool_vdf
            .parent()
            .ok_or_else(|| {
                SteamUtilError::VdfMissingEntry("Parent directory not found".to_string())
            })?
            .to_path_buf();

        // Extract directory name from the path
        let directory_name = path
            .file_name()
            .and_then(|o| o.to_str())
            .ok_or_else(|| SteamUtilError::VdfMissingEntry("Directory name not found".to_string()))?
            .to_string();

        // Extract internal name, display name, from_os_list, and to_os_list from the compatibility tool object
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
            .and_then(|o| Option::from(o.to_string()))
            .ok_or_else(|| SteamUtilError::VdfMissingEntry("Display name not found".to_string()))?;

        let from_os_list = internal_value
            .get("from_oslist")
            .and_then(|o| o.first())
            .and_then(|o| o.get_str())
            .and_then(|o| Option::from(o.to_string()))
            .ok_or_else(|| SteamUtilError::VdfMissingEntry("From OS list not found".to_string()))?;

        let to_os_list = internal_value
            .get("to_oslist")
            .and_then(|o| o.first())
            .and_then(|o| o.get_str())
            .and_then(|o| Option::from(o.to_string()))
            .ok_or_else(|| SteamUtilError::VdfMissingEntry("To OS list not found".to_string()))?;

        // Create a CompatibilityTool struct and return it
        let steam_compat_tool = CompatibilityTool {
            path,
            directory_name,
            internal_name,
            display_name,
            from_os_list,
            to_os_list,
        };
        Ok(steam_compat_tool)
    }

    pub fn list_compatibility_tools(&self) -> Result<Vec<CompatibilityTool>, SteamUtilError> {
        let compatibility_tools_directory = self.get_steam_compatibility_tools_directory();

        let compat_tools: Vec<CompatibilityTool> = fs::read_dir(compatibility_tools_directory)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|x| {
                x.metadata().unwrap().is_dir() && x.path().join("compatibilitytool.vdf").exists()
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
            SteamUtilError::VdfParsingError(steam_config_file.to_str().unwrap().to_string())
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

    /// Lists library folders.
    pub fn list_library_folders(&self) -> Result<Vec<PathBuf>, SteamUtilError> {
        let steam_apps_directory = self.steam_path.join("steamapps");

        if !steam_apps_directory.exists() {
            return Err(SteamUtilError::SteamAppsDirectoryNotFound);
        }

        let library_folders_vdf_file = self.steam_path.join("steamapps").join("libraryfolders.vdf");

        if !library_folders_vdf_file.exists() {
            return Err(SteamUtilError::LibraryFoldersVdfNotFound);
        }

        let library_folders_vdf = fs::read_to_string(&library_folders_vdf_file)
            .map_err(|err| SteamUtilError::VdfParsingError(err.to_string()))
            .unwrap();
        let vdf = Vdf::parse(&library_folders_vdf)
            .map_err(|err| SteamUtilError::VdfParsingError(err.to_string()))
            .unwrap();
        let app_state_obj = vdf.value.get_obj().unwrap();

        let mut library_folders: Vec<PathBuf> = Vec::new();

        for value in app_state_obj.values() {
            let key_obj = value.first().and_then(|o| o.get_obj()).ok_or_else(|| {
                SteamUtilError::VdfMissingEntry("Fail to retrieve entry object".to_string())
            })?;
            let path = key_obj
                .get("path")
                .and_then(|o| o.first())
                .and_then(|o| o.get_str())
                .ok_or_else(|| {
                    SteamUtilError::VdfMissingEntry("Fail to retrieve path".to_string())
                })?
                .to_string();
            if !path.is_empty() {
                library_folders.push(PathBuf::from(path));
            }
        }

        Ok(library_folders)
    }

    /// Lists the installed games across all library folders.
    pub fn list_installed_games(&self) -> Result<Vec<SteamApp>, SteamUtilError> {
        // todo: problem is this function can also return partial results because one library folder might be broken but the others might still work properly
        let mut apps: Vec<SteamApp> = Vec::new();
        match self.list_library_folders() {
            Ok(library_folders) => {
                for library_folder in library_folders {
                    let library_folder = library_folder.join("steamapps");
                    if !library_folder.exists() {
                        error!(
                            "Library folder {} does not exist",
                            library_folder.to_str().unwrap()
                        );
                        continue;
                    }
                    match &mut self.find_installed_games(library_folder.clone()) {
                        Ok(steam_apps) => apps.append(steam_apps),
                        Err(err) => {
                            error!(
                                "Failed to find installed games in library folder {}: {}",
                                &library_folder.to_str().unwrap(),
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
        let apps: Vec<SteamApp> = fs::read_dir(steam_apps_directory)
            .map_err(|_err| SteamUtilError::SteamAppsDirectoryNotFound)
            .unwrap()
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
            .and_then(|f| Option::from(f.to_string()))
            .ok_or_else(|| SteamUtilError::VdfMissingEntry("name".to_string()))?;
        Ok(SteamApp { app_id, name })
    }
}

impl Display for SteamUtilError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            SteamUtilError::HomeDirectoryNotFound => write!(f, "Home directory not found"),
            SteamUtilError::SteamDirectoryNotFound => write!(f, "Steam directory not found"),
            SteamUtilError::CompatibilityToolsDirectoryCreationFailed => {
                write!(
                    f,
                    "Steam compatibility tools directory could not be created!"
                )
            }
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

    // Helper function to create a test Steam directory with required files
    fn create_test_steam_directory() -> TempDir {
        let steam_dir = tempdir().expect("Failed to create temporary directory");
        let root_dir = steam_dir.path().join("root");
        let compatibility_tools_dir = root_dir.join("compatibilitytools.d");
        let config_dir = root_dir.join("config");
        let config_file = config_dir.join("config.vdf");
        let steamapps_dir = root_dir.join("steamapps");

        // Create necessary directories
        fs::create_dir_all(&compatibility_tools_dir)
            .expect("Failed to create compatibility tools directory");
        fs::create_dir_all(&config_dir).expect("Failed to create config directory");
        fs::create_dir_all(&steamapps_dir).expect("Failed to create steamapps directory");

        // Create compatibility tool VDF files
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

        // Create Steam config file
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
                                "730"
                                {
                                    "name"		"Sample-Compatibility-Tool-1"
                                    "config"		""
                                    "priority"		"250"
                                }
                                "1145360"
                                {
                                    "name"		"Sample-Compatibility-Tool-2"
                                    "config"		""
                                    "priority"		"250"
                                }
                            }
                        }
                    }
                }
            }
            "#,
        )
        .expect("Failed to write Steam config file");

        // Create library folders VDF file
        let library_folders_vdf_file = root_dir.join("steamapps").join("libraryfolders.vdf");
        fs::write(
            library_folders_vdf_file,
            format!(
                r#""libraryfolders"
                {{
                    "0"
                    {{
                        "path"		"{}"
                        "label"		""
                        "contentid" ""
                        "apps" {{
                            "730"   "1234567890"
                            "1145360"   "987654321"
                        }}
                    }}
                }}
                "#,
                root_dir.as_path().display() // Path to the temporary directory within libraryfolders
            ),
        )
        .expect("Failed to write library folders VDF file");

        // Create app manifest files
        let app_manifest_1 = steamapps_dir.join("appmanifest_730.acf");
        fs::write(
            app_manifest_1,
            r#""AppState"
            {
                "appid"		"730"
                "name"		"Counter-Strike: Global Offensive"
            }
            "#,
        )
        .expect("Failed to write app manifest file");

        let app_manifest_2 = steamapps_dir.join("appmanifest_1145360.acf");
        fs::write(
            app_manifest_2,
            r#""AppState"
            {
                "appid"		"1145360"
                "name"		"Hades"
            }
            "#,
        )
        .expect("Failed to write app manifest file");

        steam_dir
    }

    #[test]
    fn test_list_compatibility_tools() {
        // Create emulated Steam directory for the test
        let steam_dir = create_test_steam_directory();
        let steam_util = SteamUtil::new(steam_dir.path().join("root").to_path_buf());

        let result = steam_util.list_compatibility_tools();
        assert!(result.is_ok());
        let compat_tools = result.unwrap();
        assert_eq!(compat_tools.len(), 2);
        assert_eq!(compat_tools[0].display_name, "Sample Compatibility Tool 2");
        assert_eq!(compat_tools[1].display_name, "Sample Compatibility Tool 1");
    }

    #[test]
    fn test_get_compatibility_tools_mappings() {
        // Create emulated Steam directory for the test
        let steam_dir = create_test_steam_directory();
        let steam_util = SteamUtil::new(steam_dir.path().join("root").to_path_buf());

        let result = steam_util.get_compatibility_tools_mappings();
        assert!(result.is_ok());
        let compat_tools_mappings = result.unwrap();
        assert_eq!(compat_tools_mappings.len(), 2);
    }

    #[test]
    fn test_list_installed_games() {
        // Create emulated Steam directory for the test
        let steam_dir = create_test_steam_directory();
        let steam_util = SteamUtil::new(steam_dir.path().join("root").to_path_buf());

        let result = steam_util.list_installed_games();
        assert!(result.is_ok());
        let installed_games = result.unwrap();
        assert_eq!(installed_games.len(), 2);
        assert_eq!(installed_games[0].name, "Hades");
        assert_eq!(installed_games[1].name, "Counter-Strike: Global Offensive");
    }
}

use std::collections::HashMap;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::fs;
use std::path::PathBuf;
use std::{env, fmt};

use keyvalues_parser::Vdf;
use serde::Serialize;

#[derive(Debug)]
pub enum SteamUtilError {
    HomeDirectoryNotFound,
    SteamDirectoryNotFound,
    CompatibilityToolsDirectoryNotFound,
    SteamAppsDirectoryNotFound,
    SteamConfigFileNotFound,
    VdfParsingError(String),
}

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

    pub fn find() -> Result<Self, SteamUtilError> {
        let home_path = env::var_os("HOME")
            .or_else(|| env::var_os("USERPROFILE"))
            .ok_or(SteamUtilError::HomeDirectoryNotFound)
            .map(PathBuf::from)?;

        let steam_home = home_path.join(".steam");
        if steam_home.exists() {
            Ok(Self {
                steam_path: steam_home,
            })
        } else {
            Err(SteamUtilError::SteamDirectoryNotFound)
        }
    }

    pub fn get_steam_compatibility_tools_directory(&self) -> PathBuf {
        self.steam_path.join("root").join("compatibilitytools.d")
    }

    pub fn read_compatibility_tool_from_vdf_path(
        &self,
        compat_tool_vdf: &PathBuf,
    ) -> Result<CompatibilityTool, SteamUtilError> {
        let vdf_text = fs::read_to_string(compat_tool_vdf)
            .map_err(|err| SteamUtilError::VdfParsingError(err.to_string()))
            .unwrap();
        let vdf = Vdf::parse(&vdf_text)
            .map_err(|err| SteamUtilError::VdfParsingError(err.to_string()))
            .unwrap();

        let compat_tool_obj = vdf
            .value
            .get_obj()
            .unwrap()
            .values()
            .next()
            .unwrap()
            .get(0)
            .unwrap()
            .get_obj()
            .unwrap();

        let path = compat_tool_vdf //fixme: compat tool vdf has a path key, we can probably use that to resolve
            .parent()
            .unwrap()
            .to_path_buf();
        let directory_name = path.file_name().unwrap().to_str().unwrap().to_string();
        let internal_name = compat_tool_obj.keys().next().unwrap().to_string();
        let internal_value = compat_tool_obj
            .values()
            .next()
            .unwrap()
            .get(0)
            .unwrap()
            .get_obj()
            .unwrap();
        let display_name = internal_value
            .get("display_name")
            .unwrap()
            .get(0)
            .unwrap()
            .get_str()
            .unwrap()
            .to_string();
        let from_os_list = internal_value
            .get("from_oslist")
            .unwrap()
            .get(0)
            .unwrap()
            .get_str()
            .unwrap()
            .to_string();
        let to_os_list = internal_value
            .get("to_oslist")
            .unwrap()
            .get(0)
            .unwrap()
            .get_str()
            .unwrap()
            .to_string();

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
        if !compatibility_tools_directory.exists() {
            return Err(SteamUtilError::CompatibilityToolsDirectoryNotFound);
        }

        let compat_tools: Vec<CompatibilityTool> = fs::read_dir(&compatibility_tools_directory)
            .map_err(|err| SteamUtilError::CompatibilityToolsDirectoryNotFound)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|x| {
                x.metadata().unwrap().is_dir() && x.path().join("compatibilitytool.vdf").exists()
            })
            .map(|x| {
                self.read_compatibility_tool_from_vdf_path(&x.path().join("compatibilitytool.vdf"))
                    .unwrap()
            })
            .collect();

        Ok(compat_tools)
    }

    pub fn get_compatibility_tools_mappings(&self) -> Result<HashMap<u64, String>, SteamUtilError> {
        let steam_config_file = self
            .steam_path
            .join("root")
            .join("config")
            .join("config.vdf");

        if !steam_config_file.exists() {
            return Err(SteamUtilError::SteamConfigFileNotFound);
        }

        let mut compatibility_tools_mappings: HashMap<u64, String> = HashMap::new();
        if let Ok(config) = fs::read_to_string(&steam_config_file) {
            if let Ok(config_vdf) = Vdf::parse(&config) {
                let software_vdf_obj = config_vdf
                    .value
                    .get_obj()
                    .unwrap()
                    .get("Software")
                    .unwrap()
                    .get(0)
                    .unwrap()
                    .get_obj()
                    .unwrap();
                let compat_tools_mappings = software_vdf_obj
                    .get("Valve")
                    .or(software_vdf_obj.get("valve"))
                    .unwrap()
                    .get(0)
                    .unwrap()
                    .get_obj()
                    .unwrap()
                    .get("Steam")
                    .unwrap()
                    .get(0)
                    .unwrap()
                    .get_obj()
                    .unwrap()
                    .get("CompatToolMapping")
                    .unwrap()
                    .get(0)
                    .unwrap()
                    .get_obj()
                    .unwrap();
                for (key, value) in compat_tools_mappings {
                    let key: u64 = key.parse().unwrap();
                    let key_obj = value.get(0).unwrap().get_obj().unwrap();
                    let compat_tool_name = key_obj
                        .get("name")
                        .unwrap()
                        .get(0)
                        .unwrap()
                        .get_str()
                        .unwrap()
                        .to_string();
                    if !compat_tool_name.is_empty() {
                        compatibility_tools_mappings.insert(key, compat_tool_name);
                    }
                }
            } else {
                return Err(SteamUtilError::VdfParsingError(
                    steam_config_file.to_str().unwrap().to_string(),
                ));
            }
        } else {
            return Err(SteamUtilError::SteamConfigFileNotFound);
        }

        Ok(compatibility_tools_mappings)
    }

    pub fn list_installed_games(&self) -> Result<Vec<SteamApp>, SteamUtilError> {
        let steam_apps_directory = self.steam_path.join("root").join("steamapps");

        if !steam_apps_directory.exists() {
            return Err(SteamUtilError::SteamAppsDirectoryNotFound);
        }

        let apps: Vec<SteamApp> = fs::read_dir(&steam_apps_directory)
            .map_err(|err| SteamUtilError::SteamAppsDirectoryNotFound)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|x| x.path().extension().unwrap_or_default().eq("acf"))
            .map(|file| {
                let app_manifest = fs::read_to_string(file.path())
                    .map_err(|err| SteamUtilError::VdfParsingError(err.to_string()))
                    .unwrap();
                let vdf = Vdf::parse(&app_manifest)
                    .map_err(|err| SteamUtilError::VdfParsingError(err.to_string()))
                    .unwrap();
                let app_state_obj = vdf.value.get_obj().unwrap();
                let app_id: u64 = app_state_obj
                    .get("appid")
                    .unwrap()
                    .get(0)
                    .unwrap()
                    .get_str()
                    .unwrap()
                    .parse()
                    .unwrap();
                let name: String = app_state_obj
                    .get("name")
                    .unwrap()
                    .get(0)
                    .unwrap()
                    .get_str()
                    .unwrap()
                    .to_string();
                SteamApp { app_id, name }
            })
            .collect();

        Ok(apps)
    }
}

impl Display for SteamUtilError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            SteamUtilError::HomeDirectoryNotFound => write!(f, "Home directory not found"),
            SteamUtilError::SteamDirectoryNotFound => write!(f, "Steam directory not found"),
            SteamUtilError::CompatibilityToolsDirectoryNotFound => {
                write!(f, "Steam compatibility tools directory not found")
            }
            SteamUtilError::SteamAppsDirectoryNotFound => {
                write!(f, "Steam apps directory not found")
            }
            SteamUtilError::SteamConfigFileNotFound => write!(f, "Steam config file not found"),
            SteamUtilError::VdfParsingError(msg) => write!(f, "Failed to parse VDF file: {}", msg),
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
            &compat_tool_1_vdf,
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
            &compat_tool_2_vdf,
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
            &config_file,
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

        // Create app manifest files
        let app_manifest_1 = steamapps_dir.join("appmanifest_730.acf");
        fs::write(
            &app_manifest_1,
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
            &app_manifest_2,
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
        let steam_util = SteamUtil::new(steam_dir.path().to_path_buf());

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
        let steam_util = SteamUtil::new(steam_dir.path().to_path_buf());

        let result = steam_util.get_compatibility_tools_mappings();
        assert!(result.is_ok());
        let compat_tools_mappings = result.unwrap();
        assert_eq!(compat_tools_mappings.len(), 2);
    }

    #[test]
    fn test_list_installed_games() {
        // Create emulated Steam directory for the test
        let steam_dir = create_test_steam_directory();
        let steam_util = SteamUtil::new(steam_dir.path().to_path_buf());

        let result = steam_util.list_installed_games();
        assert!(result.is_ok());
        let installed_games = result.unwrap();
        assert_eq!(installed_games.len(), 2);
        assert_eq!(installed_games[0].name, "Hades");
        assert_eq!(installed_games[1].name, "Counter-Strike: Global Offensive");
    }
}

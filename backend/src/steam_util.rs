use std::{env, fmt, fs};
use std::collections::HashMap;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::path::PathBuf;
use keyvalues_parser::Vdf;
use log::{error, info};
use serde::Serialize;

#[derive(Debug)]
pub(crate) enum SteamUtilError {
    HomeDirectoryNotFound,
    SteamDirectoryNotFound,
    CompatibilityToolsDirectoryNotFound,
    SteamAppsDirectoryNotFound,
    SteamConfigFileNotFound,
}

pub(crate) struct SteamUtil {
    steam_path: PathBuf,
}

#[derive(Serialize)]
pub struct CompatibilityTool {
    pub path: PathBuf,
    pub name: String,
    pub internal_name: String,
    pub display_name: String,
    pub from_os_list: String,
    pub to_os_list: String,
}

#[derive(Serialize)]
pub struct SteamApp {
    //path: PathBuf, todo:
    pub app_id: u64,
    pub name: String,
}

impl SteamUtil {
    pub(crate) fn new(steam_home: PathBuf) -> Self {
        Self { steam_path: steam_home }
    }

    pub(crate) fn find() -> Result<Self, SteamUtilError> {
        let home_path = if let Some(home_dir) = env::var_os("HOME") {
            PathBuf::from(home_dir)
        } else if let Some(home_dir) = env::var_os("USERPROFILE") {
            PathBuf::from(home_dir)
        } else {
            return Err(SteamUtilError::HomeDirectoryNotFound);
        };

        let steam_home = home_path.join(".steam");
        if steam_home.exists() {
            info!("Steam home directory: {:?}", steam_home);
            Ok(Self {
                steam_path: steam_home,
            })
        } else {
            Err(SteamUtilError::SteamDirectoryNotFound)
        }
    }

    pub fn get_steam_compatibility_tools_directory(&self) -> PathBuf {
        return self.steam_path.join("root").join("compatibilitytools.d")
    }

    pub(crate) fn list_compatibility_tools(&self) -> Result<Vec<CompatibilityTool>, SteamUtilError> {
        let compatibility_tools_directory = self.get_steam_compatibility_tools_directory();
        if !compatibility_tools_directory.exists() {
            return Err(SteamUtilError::CompatibilityToolsDirectoryNotFound);
        }

        let mut compatibility_tools = Vec::new();

        if let Ok(entries) = fs::read_dir(&compatibility_tools_directory) {
            for entry in entries {
                if let Ok(entry) = entry {
                    if let Ok(metadata) = entry.metadata() {
                        if metadata.is_dir() {
                            let compat_tool_vdf = entry.path().join("compatibilitytool.vdf");
                            if compat_tool_vdf.exists() {
                                if let Ok(vdf_file) = fs::read_to_string(&compat_tool_vdf) {
                                    if let Ok(vdf) = Vdf::parse(&vdf_file) {
                                        if let Some(compat_tool_obj) = vdf.value.get_obj().and_then(|obj| obj.values().next()).and_then(|value| value.get(0)).and_then(|value| value.get_obj()) {
                                            let path = entry.path();
                                            let name: String = entry.file_name().to_str().unwrap().parse().unwrap();
                                            let internal_name: String = compat_tool_obj.keys().next().unwrap().parse().unwrap();
                                            let internal_value = compat_tool_obj.values().next().unwrap().get(0).unwrap().get_obj().unwrap();
                                            let display_name: String = internal_value.get("display_name").unwrap().get(0).unwrap().get_str().unwrap().parse().unwrap();
                                            let from_os_list: String = internal_value.get("from_oslist").unwrap().get(0).unwrap().get_str().unwrap().parse().unwrap();
                                            let to_os_list: String = internal_value.get("to_oslist").unwrap().get(0).unwrap().get_str().unwrap().parse().unwrap();

                                            let steam_compat_tool = CompatibilityTool {
                                                path,
                                                name,
                                                internal_name,
                                                display_name,
                                                from_os_list,
                                                to_os_list,
                                            };
                                            compatibility_tools.push(steam_compat_tool);
                                        }
                                    } else {
                                        // todo: something went wrong
                                    }
                                } else {
                                    // todo: something went wrong
                                }
                            } else {
                                error!("Invalid compatibility tool installation: {}", entry.path().to_string_lossy());
                            }
                        }
                    }
                }
            }
        } else {
            error!("Failed to read the compatibility tools directory: {}", compatibility_tools_directory.to_string_lossy());
        }

        Ok(compatibility_tools)
    }

    pub(crate) fn get_compatibility_tools_mappings(&self) -> Result<HashMap<u64, String>, SteamUtilError> {
        let steam_config_file = self.steam_path.join("root").join("config").join("config.vdf");

        if !steam_config_file.exists() {
            return Err(SteamUtilError::SteamConfigFileNotFound);
        }
        let mut compatibility_tools_mappings: HashMap<u64, String> = HashMap::new();
        if let Ok(config) = fs::read_to_string(&steam_config_file) {
            if let Ok(config_vdf) = Vdf::parse(&config) {
                let soft = config_vdf.value.get_obj().unwrap().get("Software").unwrap().get(0).unwrap().get_obj().unwrap();
                let valve = soft.get("Valve").unwrap().get(0).unwrap().get_obj().unwrap(); // fixme: https://github.com/DavidoTek/ProtonUp-Qt/issues/226
                let steam = valve.get("Steam").unwrap().get(0).unwrap().get_obj().unwrap();
                let compat_tools_mappings = steam.get("CompatToolMapping").unwrap().get(0).unwrap().get_obj().unwrap();
                for x in compat_tools_mappings.keys() {
                    let key: u64 = x.parse().unwrap();
                    let key_obj = compat_tools_mappings.get_key_value(x).unwrap().1.get(0).unwrap().get_obj().unwrap();
                    let compat_tool_name: String = key_obj.get("name").unwrap().get(0).unwrap().get_str().unwrap().parse().unwrap();
                    if !compat_tool_name.is_empty() {
                        compatibility_tools_mappings.insert(key, compat_tool_name);
                    }
                }
            } else {
                // todo: something went wrong
            }
        } else {
            // todo: something went wrong
        }


        Ok(compatibility_tools_mappings)
    }

    pub(crate) fn list_installed_games(&self) -> Result<Vec<SteamApp>, SteamUtilError> {
        let steam_apps_directory = self.steam_path.join("root").join("steamapps");

        if !steam_apps_directory.exists() {
            return Err(SteamUtilError::SteamAppsDirectoryNotFound);
        }

        let mut steam_apps: Vec<SteamApp> = Vec::new();

        if let Ok(entries) = fs::read_dir(&steam_apps_directory) {
            for entry in entries {
                if let Ok(entry) = entry {
                    if let Ok(metadata) = entry.metadata() {
                        if metadata.is_file() && entry.file_name().to_os_string().to_str().unwrap().ends_with(".acf") {
                            if let Ok(app_manifest) = fs::read_to_string(&entry.path()) {
                                if let Ok(app_manifest_vdf) = Vdf::parse(&app_manifest) {
                                    let app_state_obj = app_manifest_vdf.value.get_obj().unwrap();
                                    let app_id: u64 = app_state_obj.get("appid").unwrap().get(0).unwrap().get_str().unwrap().parse().unwrap();
                                    let name: String = app_state_obj.get("name").unwrap().get(0).unwrap().get_str().unwrap().parse().unwrap();

                                    steam_apps.push(SteamApp {
                                        app_id,
                                        name,
                                    })
                                } else {
                                    // todo: something went wrong
                                }
                            } else {
                                // todo: something went wrong
                            }
                        }
                    }
                }
            }
        } else {
            error!("Failed to read the compatibility tools directory: {}", steam_apps_directory.to_string_lossy());
        }

        Ok(steam_apps)
    }
}

impl Display for SteamUtilError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            SteamUtilError::HomeDirectoryNotFound => {
                error!("Unable to determine the home directory.");
                write!(f, "Home directory not found")
            }
            SteamUtilError::SteamDirectoryNotFound => {
                error!("Unable to determine the Steam home directory.");
                write!(f, "Steam directory not found")
            }
            SteamUtilError::CompatibilityToolsDirectoryNotFound => {
                error!("Unable to determine the Steam compatibility tools directory.");
                write!(f, "Steam compatibility tools directory not found")
            }
            SteamUtilError::SteamAppsDirectoryNotFound => {
                error!("Unable to determine the Steam apps directory.");
                write!(f, "Steam apps directory not found")
            }
            SteamUtilError::SteamConfigFileNotFound => {
                error!("Unable to determine the Steam config file.");
                write!(f, "Steam config file not found")
            }
        }
    }
}

impl Error for SteamUtilError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}
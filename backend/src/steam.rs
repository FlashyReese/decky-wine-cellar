use std::fs;
use std::path::PathBuf;
use log::error;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct SteamCompatibilityTool {
    pub(crate) name: String,
    pub(crate) internal_name: String,
    pub(crate) display_name: String,
    pub(crate) version: Option<String>,
    pub(crate) path: String,
    pub(crate) requires_restart: bool,
}

pub fn get_installed_compatibility_tools(steam_compat_directory: &PathBuf) -> Vec<SteamCompatibilityTool> {
    let mut result = Vec::new();

    if let Ok(entries) = fs::read_dir(steam_compat_directory) {
        for entry in entries {
            if let Ok(entry) = entry {
                if let Ok(metadata) = entry.metadata() {
                    if metadata.is_dir() {
                        let compat_tool_vdf = entry.path().join("compatibilitytool.vdf");
                        let version_path = entry.path().join("version");

                        if compat_tool_vdf.exists() {
                            let vdf_file = fs::read_to_string(&compat_tool_vdf).ok().unwrap();
                            let version = fs::read_to_string(&version_path)
                                .ok()
                                .and_then(|content| {
                                    content
                                        .split(' ')
                                        .next()
                                        .map(|version| version.trim().to_owned())
                                });


                            let vdf = keyvalues_parser::Vdf::parse(&vdf_file).unwrap();
                            let compat_tool_obj = vdf.value.get_obj().unwrap().values().next().unwrap().get(0).unwrap().get_obj().unwrap();
                            let internal_name = compat_tool_obj.keys().next().unwrap();
                            let display_name = compat_tool_obj.values().next().unwrap().get(0).unwrap().get_obj().unwrap().get("display_name").unwrap().get(0).unwrap();

                            let steam_compat_tool = SteamCompatibilityTool {
                                name: entry.file_name().to_str().unwrap().to_string(),
                                internal_name: internal_name.to_string(),
                                display_name: display_name.to_owned().unwrap_str().to_string(),
                                version,
                                path: entry.path().to_str().unwrap().to_string(),
                                requires_restart: false,
                            };
                            result.push(steam_compat_tool);
                        }
                    }
                }
            }
        }
    } else {
        error!("Failed to read the compat_directory: {}", steam_compat_directory.to_str().unwrap());
    }

    result
}
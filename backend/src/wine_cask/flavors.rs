use std::{env, fs};
use std::path::PathBuf;
use std::time::SystemTime;
use log::info;
use serde::{Deserialize, Serialize};
use crate::github_util;
use crate::github_util::Release;
use crate::wine_cask::wine_cask::WineCask;

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub enum CompatibilityToolFlavor {
    Unknown,
    ProtonGE,
    SteamTinkerLaunch,
    Luxtorpeda,
    Boxtron,
}

impl std::fmt::Display for CompatibilityToolFlavor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CompatibilityToolFlavor::Unknown => write!(f, "Unknown"),
            CompatibilityToolFlavor::ProtonGE => write!(f, "ProtonGE"),
            CompatibilityToolFlavor::SteamTinkerLaunch => write!(f, "SteamTinkerLaunch"),
            CompatibilityToolFlavor::Luxtorpeda => write!(f, "Luxtorpeda"),
            CompatibilityToolFlavor::Boxtron => write!(f, "Boxtron"),
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Flavor {
    pub flavor: CompatibilityToolFlavor,
    pub installed: Vec<SteamCompatibilityTool>,
    pub not_installed: Vec<Release>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct SteamCompatibilityTool {
    pub path: String,
    //pub directory_name: String,
    pub display_name: String,
    pub internal_name: String,
    pub used_by_games: Vec<String>,
    pub requires_restart: bool,
    //pub r#virtual: bool,
    //pub virtual_original: String, // Display name or Internal name or name?
}

impl WineCask {
    pub async fn get_flavors(&self, installed_compatibility_tools: Vec<SteamCompatibilityTool>, renew_cache: bool) -> Vec<Flavor> {
        let mut flavors = Vec::new();

        let proton_ge_flavor = self
            .get_flavor(
                &installed_compatibility_tools,
                CompatibilityToolFlavor::ProtonGE,
                "GloriousEggroll",
                "proton-ge-custom",
                renew_cache,
            )
            .await;
        /*let steam_tinker_launch_flavor = self
            .get_flavor(
                &installed_compatibility_tools,
                CompatibilityToolFlavor::SteamTinkerLaunch,
                "sonic2kk",
                "steamtinkerlaunch",
                renew_cache,
            )
            .await;*/
        let luxtorpeda_flavor = self
            .get_flavor(
                &installed_compatibility_tools,
                CompatibilityToolFlavor::Luxtorpeda,
                "luxtorpeda-dev",
                "luxtorpeda",
                renew_cache,
            )
            .await;
        let boxtron_flavor = self
            .get_flavor(
                &installed_compatibility_tools,
                CompatibilityToolFlavor::Boxtron,
                "dreamer",
                "boxtron",
                renew_cache,
            )
            .await;

        flavors.push(proton_ge_flavor);
        //flavors.push(steam_tinker_launch_flavor); fixme: we need to have a special installation process for this.
        flavors.push(luxtorpeda_flavor);
        flavors.push(boxtron_flavor);

        flavors
    }

    async fn get_flavor(&self, installed_compatibility_tools: &Vec<SteamCompatibilityTool>, compatibility_tool_flavor: CompatibilityToolFlavor, owner: &str, repository: &str, renew_cache: bool, ) -> Flavor {
        if let Some(github_releases) = self.get_releases(owner, repository, renew_cache).await {
            let installed: Vec<SteamCompatibilityTool> = installed_compatibility_tools
                .iter()
                .filter(|tool| {
                    github_releases.iter().any(|gh| {
                        if compatibility_tool_flavor == CompatibilityToolFlavor::ProtonGE {
                            tool.internal_name == gh.tag_name || tool.display_name == gh.tag_name
                        } else {
                            tool.display_name
                                == compatibility_tool_flavor.to_string() + " " + &gh.tag_name
                                || tool.internal_name
                                == compatibility_tool_flavor.to_string() + &gh.tag_name
                        }
                    })
                })
                .cloned()
                .collect();
            let not_installed: Vec<Release> = github_releases
                .iter()
                .filter(|gh| {
                    !installed.iter().any(|tool| {
                        if compatibility_tool_flavor == CompatibilityToolFlavor::ProtonGE {
                            tool.internal_name == gh.tag_name || tool.display_name == gh.tag_name
                        } else {
                            tool.display_name
                                == compatibility_tool_flavor.to_string() + " " + &gh.tag_name
                                || tool.internal_name
                                == compatibility_tool_flavor.to_string() + &gh.tag_name
                        }
                    })
                })
                .cloned()
                .collect();

            Flavor {
                flavor: compatibility_tool_flavor,
                installed,
                not_installed,
            }
        } else {
            Flavor {
                flavor: compatibility_tool_flavor,
                installed: Vec::new(),
                not_installed: Vec::new(),
            }
        }
    }

    async fn get_releases(&self, owner: &str, repository: &str, renew_cache: bool) -> Option<Vec<Release>> {
        // Use a named constant for seconds in a day
        const SECONDS_IN_A_DAY: u64 = 84_600;

        let path = env::var("DECKY_PLUGIN_RUNTIME_DIR").unwrap_or("/tmp/".parse().unwrap());

        let file_name = format!("github_releases_{}_{}_cache.json", owner, repository);
        let cache_file = PathBuf::from(path).join(&file_name);

        if !renew_cache && cache_file.exists() && cache_file.is_file() {
            let metadata = fs::metadata(&cache_file).ok()?;
            let modified = metadata.modified().ok()?;

            // Calculate the duration between the current time and the file modification time
            let now = SystemTime::now();
            let duration = now.duration_since(modified).ok()?;

            if duration.as_secs() < SECONDS_IN_A_DAY {
                let string = fs::read_to_string(&cache_file).ok()?;
                let github_releases: Vec<Release> = serde_json::from_str(&string).ok()?;
                return Some(github_releases);
            } else {
                info!("Cache file is older than 1 day. Fetching new releases.");
            }
        }

        let github_releases = match github_util::list_all_releases(owner, repository).await {
            Ok(releases) => {
                let json = serde_json::to_string(&releases).ok()?;
                fs::write(&cache_file, json).ok()?;
                releases
            }
            Err(_) => {
                if cache_file.exists() && cache_file.is_file() {
                    let string = fs::read_to_string(&cache_file).ok()?;
                    let github_releases: Vec<Release> = serde_json::from_str(&string).ok()?;
                    info!("Unable to fetch new releases. Using cached releases.");
                    github_releases
                } else {
                    info!("Unable to fetch new releases. No cached releases found.");
                    return None;
                }
            }
        };

        Some(github_releases)
    }
}
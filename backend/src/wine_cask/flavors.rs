use crate::github_util;
use crate::github_util::Release;
use crate::wine_cask::app::WineCask;
use log::{error, info, warn};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use std::{env, fs};

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
    pub releases: Vec<Release>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct SteamCompatibilityTool {
    pub path: String,
    //pub directory_name: String,
    pub display_name: String,
    pub internal_name: String,
    pub used_by_games: Vec<String>,
    pub requires_restart: bool,
    pub flavor: CompatibilityToolFlavor,
    pub github_release: Option<Release>,
    //pub r#virtual: bool,
    //pub virtual_original: String, // Display name or Internal name or name?
}

// SteamClient.Apps.GetAvailableCompatTools()
#[derive(Serialize, Deserialize, Clone)]
pub struct SteamClientCompatToolInfo {
    #[serde(rename = "strToolName")]
    pub str_tool_name: String,
    #[serde(rename = "strDisplayName")]
    pub str_display_name: String,
}

impl WineCask {
    pub async fn get_flavors(&self, renew_cache: bool) -> Vec<Flavor> {
        let mut flavors = Vec::new();

        let proton_ge_flavor = self
            .get_flavor(
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
                CompatibilityToolFlavor::Luxtorpeda,
                "luxtorpeda-dev",
                "luxtorpeda",
                renew_cache,
            )
            .await;
        let boxtron_flavor = self
            .get_flavor(
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

    async fn get_flavor(
        &self,
        compatibility_tool_flavor: CompatibilityToolFlavor,
        owner: &str,
        repository: &str,
        renew_cache: bool,
    ) -> Flavor {
        if let Some(github_releases) = self.get_releases(owner, repository, renew_cache).await {
            Flavor {
                flavor: compatibility_tool_flavor,
                releases: github_releases,
            }
        } else {
            Flavor {
                flavor: compatibility_tool_flavor,
                releases: Vec::new(),
            }
        }
    }

    pub async fn update_compatibility_tools_and_available_flavors(&self) {
        let mut app_state = self.app_state.lock().await;
        app_state.available_flavors.clear();
        for flavor in app_state.flavors.clone() {
            let mut installed_compatibility_tools = app_state.installed_compatibility_tools.clone();
            let compatibility_tool_flavor = flavor.flavor.clone();
            let github_releases = flavor.releases.clone();

            for steam_compat_tool in &mut installed_compatibility_tools {
                if let Some(release) = github_releases.iter().find(|gh| {
                    if compatibility_tool_flavor == CompatibilityToolFlavor::ProtonGE {
                        steam_compat_tool.internal_name == gh.tag_name
                            || steam_compat_tool.display_name == gh.tag_name
                    } else {
                        steam_compat_tool.display_name
                            == compatibility_tool_flavor.to_string() + " " + &gh.tag_name
                            || steam_compat_tool.internal_name
                                == compatibility_tool_flavor.to_string() + &gh.tag_name
                    }
                }) {
                    steam_compat_tool.flavor = compatibility_tool_flavor.clone();
                    steam_compat_tool.github_release = Some(release.clone());
                }
            }

            app_state.installed_compatibility_tools = installed_compatibility_tools.clone();

            let not_installed: Vec<Release> = github_releases
                .iter()
                .filter(|gh| {
                    !installed_compatibility_tools.iter().any(|tool| {
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
            app_state.available_flavors.push(Flavor {
                flavor: compatibility_tool_flavor,
                releases: not_installed,
            });
        }
    }

    async fn get_releases(
        &self,
        owner: &str,
        repository: &str,
        renew_cache: bool,
    ) -> Option<Vec<Release>> {
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
                // Update last checked time with file last modified time
                let unix_timestamp = modified
                    .duration_since(UNIX_EPOCH)
                    .expect("Failed to calculate duration")
                    .as_secs();
                self.app_state.lock().await.updater_last_check = Some(unix_timestamp);

                let string = fs::read_to_string(&cache_file).ok()?;
                let github_releases: Vec<Release> = serde_json::from_str(&string).ok()?;

                // Check if parsing failed but data exists (cache is corrupted)
                if github_releases.is_empty() {
                    info!("Cached data is possibly corrupted or possibly missing information from outdated version. Renewing cache...");
                } else {
                    return Some(github_releases);
                }
            } else {
                info!("Cache file is older than 1 day. Fetching new releases.");
            }
        }

        let github_releases = match github_util::list_all_releases(owner, repository).await {
            Ok(releases) => {
                if releases.is_empty() {
                    error!("No releases found.");
                    return None;
                }

                // Update last checked time
                let current_time = SystemTime::now();
                let unix_timestamp = current_time
                    .duration_since(UNIX_EPOCH)
                    .expect("Failed to calculate duration")
                    .as_secs();
                self.app_state.lock().await.updater_last_check = Some(unix_timestamp);

                let json = serde_json::to_string(&releases).ok()?;
                fs::write(&cache_file, json).ok()?;
                releases
            }
            Err(_) => {
                if cache_file.exists() && cache_file.is_file() {
                    // Update last checked time with file last modified time
                    let metadata = fs::metadata(&cache_file).ok()?;
                    let modified = metadata.modified().ok()?;
                    let unix_timestamp = modified
                        .duration_since(UNIX_EPOCH)
                        .expect("Failed to calculate duration")
                        .as_secs();
                    self.app_state.lock().await.updater_last_check = Some(unix_timestamp);

                    let string = fs::read_to_string(&cache_file).ok()?;
                    let github_releases: Vec<Release> = serde_json::from_str(&string).ok()?;
                    warn!("Unable to fetch new releases. Using cached releases.");
                    github_releases
                } else {
                    error!("Unable to fetch new releases. No cached releases found.");
                    return None;
                }
            }
        };

        Some(github_releases)
    }
}

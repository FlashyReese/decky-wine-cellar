use crate::github_util;
use crate::github_util::{Release, FetchResult};
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

    /// Reads the last-modified timestamp from metadata file
    fn read_cache_metadata(path: &str, owner: &str, repository: &str) -> Option<String> {
        let metadata_file = PathBuf::from(path)
            .join(format!("github_releases_{}_{}_metadata.txt", owner, repository));

        if metadata_file.exists() {
            fs::read_to_string(metadata_file).ok()
        } else {
            None
        }
    }

    /// Writes the last-modified timestamp to metadata file
    fn write_cache_metadata(path: &str, owner: &str, repository: &str, last_modified: &str) {
        let metadata_file = PathBuf::from(path)
            .join(format!("github_releases_{}_{}_metadata.txt", owner, repository));

        if let Err(e) = fs::write(metadata_file, last_modified) {
            warn!("Failed to write cache metadata: {}", e);
        }
    }

    async fn get_releases(
        &self,
        owner: &str,
        repository: &str,
        renew_cache: bool,
    ) -> Option<Vec<Release>> {
        // Fixed: 86,400 seconds in a day (not 84,600)
        const CACHE_DURATION_SECONDS: u64 = 86_400;

        let path = env::var("DECKY_PLUGIN_RUNTIME_DIR").unwrap_or("/tmp/".parse().unwrap());
        let cache_file = PathBuf::from(&path)
            .join(format!("github_releases_{}_{}_cache.json", owner, repository));

        // Try to load cached data
        let cached_data = if cache_file.exists() && cache_file.is_file() {
            match fs::metadata(&cache_file) {
                Ok(metadata) => match metadata.modified() {
                    Ok(modified) => {
                        let now = SystemTime::now();
                        match now.duration_since(modified) {
                            Ok(duration) => match fs::read_to_string(&cache_file) {
                                Ok(string) => match serde_json::from_str::<Vec<Release>>(&string) {
                                    Ok(github_releases) if !github_releases.is_empty() => {
                                        Some((github_releases, duration.as_secs(), modified))
                                    }
                                    Ok(_) => {
                                        info!(
                                            "Cached data is empty or corrupted. Renewing cache..."
                                        );
                                        None
                                    }
                                    Err(e) => {
                                        warn!(
                                            "Failed to parse cache file ({}). Renewing cache...",
                                            e
                                        );
                                        None
                                    }
                                },
                                Err(e) => {
                                    warn!("Failed to read cache file ({}). Renewing cache...", e);
                                    None
                                }
                            },
                            Err(e) => {
                                warn!(
                                    "Invalid cache modification timestamp ({}). Renewing cache...",
                                    e
                                );
                                None
                            }
                        }
                    }
                    Err(e) => {
                        warn!(
                            "Failed to read cache file metadata timestamp ({}). Renewing cache...",
                            e
                        );
                        None
                    }
                },
                Err(e) => {
                    warn!("Failed to read cache file metadata ({}). Renewing cache...", e);
                    None
                }
            }
        } else {
            None
        };

        // Determine if we should use cache, check with conditional request, or fetch fresh
        let should_fetch_fresh = renew_cache || cached_data.is_none();

        if !should_fetch_fresh {
            if let Some((releases, cache_age, modified)) = cached_data {
                // Cache is fresh (less than CACHE_DURATION old), use it directly
                if cache_age < CACHE_DURATION_SECONDS {
                    let unix_timestamp = modified
                        .duration_since(UNIX_EPOCH)
                        .expect("Failed to calculate duration")
                        .as_secs();
                    self.app_state.lock().await.updater_last_check = Some(unix_timestamp);

                    info!("Using cached releases (age: {}s < {}s)", cache_age, CACHE_DURATION_SECONDS);
                    return Some(releases);
                }

                // Cache is stale, try conditional request with If-Modified-Since
                let last_modified = Self::read_cache_metadata(&path, owner, repository);

                info!("Cache is stale (age: {}s), checking with GitHub using If-Modified-Since", cache_age);

                match github_util::list_all_releases(owner, repository, last_modified.as_deref()).await {
                    Ok(FetchResult::NotModified) => {
                        // Cache is still valid, update last_check time and use it
                        let unix_timestamp = modified
                            .duration_since(UNIX_EPOCH)
                            .expect("Failed to calculate duration")
                            .as_secs();
                        self.app_state.lock().await.updater_last_check = Some(unix_timestamp);

                        info!("GitHub confirms cache is still valid (304 Not Modified)");
                        return Some(releases);
                    }
                    Ok(FetchResult::Modified(new_releases, new_last_modified)) => {
                        // New data available, update cache
                        if new_releases.is_empty() {
                            error!("No releases found in fresh fetch.");
                            return None;
                        }

                        let current_time = SystemTime::now();
                        let unix_timestamp = current_time
                            .duration_since(UNIX_EPOCH)
                            .expect("Failed to calculate duration")
                            .as_secs();
                        self.app_state.lock().await.updater_last_check = Some(unix_timestamp);

                        // Update cache file
                        let json = serde_json::to_string(&new_releases).ok()?;
                        if let Err(e) = fs::write(&cache_file, json) {
                            error!("Failed to write cache file: {}", e);
                        }

                        // Update metadata file with new last-modified timestamp
                        if let Some(lm) = new_last_modified {
                            Self::write_cache_metadata(&path, owner, repository, &lm);
                        }

                        info!("Updated cache with {} releases from GitHub", new_releases.len());
                        return Some(new_releases);
                    }
                    Err(e) => {
                        // Failed to check with GitHub, fall back to cached data
                        warn!("Failed to check GitHub for updates ({}), using stale cache", e);
                        let unix_timestamp = modified
                            .duration_since(UNIX_EPOCH)
                            .expect("Failed to calculate duration")
                            .as_secs();
                        self.app_state.lock().await.updater_last_check = Some(unix_timestamp);
                        return Some(releases);
                    }
                }
            }
        }

        // Fetch fresh data (no cache or renew_cache=true)
        info!("Fetching fresh releases from GitHub for {}/{}", owner, repository);

        match github_util::list_all_releases(owner, repository, None).await {
            Ok(FetchResult::Modified(releases, last_modified)) => {
                if releases.is_empty() {
                    error!("No releases found.");
                    return None;
                }

                let current_time = SystemTime::now();
                let unix_timestamp = current_time
                    .duration_since(UNIX_EPOCH)
                    .expect("Failed to calculate duration")
                    .as_secs();
                self.app_state.lock().await.updater_last_check = Some(unix_timestamp);

                // Update cache file
                let json = serde_json::to_string(&releases).ok()?;
                if let Err(e) = fs::write(&cache_file, json) {
                    error!("Failed to write cache file: {}", e);
                }

                // Update metadata file
                if let Some(lm) = last_modified {
                    Self::write_cache_metadata(&path, owner, repository, &lm);
                }

                info!("Fetched and cached {} releases", releases.len());
                Some(releases)
            }
            Ok(FetchResult::NotModified) => {
                // This shouldn't happen when we don't send If-Modified-Since
                warn!("Unexpected 304 Not Modified response without If-Modified-Since header");
                None
            }
            Err(e) => {
                // Try to fall back to cached data even if it's old
                if let Some((releases, _, modified)) = cached_data {
                    warn!("Failed to fetch releases ({}), falling back to cached data", e);
                    let unix_timestamp = modified
                        .duration_since(UNIX_EPOCH)
                        .expect("Failed to calculate duration")
                        .as_secs();
                    self.app_state.lock().await.updater_last_check = Some(unix_timestamp);
                    Some(releases)
                } else {
                    error!("Unable to fetch releases and no cache available: {}", e);
                    None
                }
            }
        }
    }
}

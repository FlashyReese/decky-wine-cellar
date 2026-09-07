use crate::github_util;
use crate::github_util::Release;
use crate::wine_cask::app::WineCask;
use log::{error, info, warn};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use std::{env, fs};

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub enum CompatibilityToolFlavor {
    Unknown,
    ProtonGE,
    ProtonCachyOS,
    SteamTinkerLaunch,
    Luxtorpeda,
    Boxtron,
}

impl std::fmt::Display for CompatibilityToolFlavor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CompatibilityToolFlavor::Unknown => write!(f, "Unknown"),
            CompatibilityToolFlavor::ProtonGE => write!(f, "ProtonGE"),
            CompatibilityToolFlavor::ProtonCachyOS => write!(f, "ProtonCachyOS"),
            CompatibilityToolFlavor::SteamTinkerLaunch => write!(f, "SteamTinkerLaunch"),
            CompatibilityToolFlavor::Luxtorpeda => write!(f, "Luxtorpeda"),
            CompatibilityToolFlavor::Boxtron => write!(f, "Boxtron"),
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct CatalogRelease {
    pub id: String,
    pub flavor: CompatibilityToolFlavor,
    pub release: Release,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Flavor {
    pub flavor: CompatibilityToolFlavor,
    pub releases: Vec<CatalogRelease>,
}

#[derive(Serialize, Deserialize, Clone)]
pub enum InstalledToolSource {
    Direct,
    Virtual,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct InstalledCompatibilityTool {
    pub id: String,
    pub path: String,
    pub directory_name: String,
    pub display_name: String,
    pub internal_name: String,
    pub used_by_games: Vec<String>,
    pub requires_restart: bool,
    pub flavor: CompatibilityToolFlavor,
    pub catalog_release_id: Option<String>,
    pub github_release: Option<Release>,
    pub source: InstalledToolSource,
    pub virtual_tool_id: Option<String>,
    pub user_label: Option<String>,
    pub can_link_to_virtual_tool: bool,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct VirtualCompatibilityTool {
    pub id: String,
    pub user_label: String,
    pub steam_internal_name: String,
    pub directory_name: String,
    pub installed_tool_id: Option<String>,
    pub current_payload_release_id: Option<String>,
    pub current_payload_name: Option<String>,
    pub current_payload_flavor: CompatibilityToolFlavor,
    pub github_release: Option<Release>,
    pub linked_source_installed_tool_id: Option<String>,
    pub linked_source_missing: bool,
    pub requires_restart: bool,
    pub used_by_games: Vec<String>,
}

// SteamClient.Settings.GetGlobalCompatTools()
#[derive(Serialize, Deserialize, Clone)]
pub struct SteamClientCompatToolInfo {
    #[serde(rename = "strToolName")]
    pub str_tool_name: String,
    #[serde(rename = "strDisplayName")]
    pub str_display_name: String,
}

pub fn catalog_release_id(flavor: &CompatibilityToolFlavor, release_id: u64) -> String {
    format!("catalog:{}:{}", flavor, release_id)
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
        flavors.push(luxtorpeda_flavor);
        flavors.push(boxtron_flavor);

        let proton_cachyos_flavor = self
            .get_flavor_with_v3_filter(
                CompatibilityToolFlavor::ProtonCachyOS,
                "CachyOS",
                "proton-cachyos",
                renew_cache,
            )
            .await;
        flavors.push(proton_cachyos_flavor);

        flavors
    }

    async fn get_flavor_with_v3_filter(
        &self,
        compatibility_tool_flavor: CompatibilityToolFlavor,
        owner: &str,
        repository: &str,
        renew_cache: bool,
    ) -> Flavor {
        let releases = self.get_releases(owner, repository, renew_cache).await;

        if let Some(releases) = releases {
            let filtered = Self::filter_v3_releases(releases);
            if filtered.is_empty() {
                return Flavor {
                    flavor: compatibility_tool_flavor,
                    releases: Vec::new(),
                };
            }
            Flavor {
                flavor: compatibility_tool_flavor.clone(),
                releases: filtered
                    .into_iter()
                    .map(|release| CatalogRelease {
                        id: catalog_release_id(&compatibility_tool_flavor, release.id),
                        flavor: compatibility_tool_flavor.clone(),
                        release,
                    })
                    .collect(),
            }
        } else {
            Flavor {
                flavor: compatibility_tool_flavor,
                releases: Vec::new(),
            }
        }
    }

    fn filter_v3_releases(releases: Vec<Release>) -> Vec<Release> {
        let allowed_arches = ["x86_64_v3"];
        releases
            .into_iter()
            .filter(|release| {
                release
                    .assets
                    .iter()
                    .any(|asset| allowed_arches.iter().any(|arch| asset.name.contains(arch)))
            })
            .collect()
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
                flavor: compatibility_tool_flavor.clone(),
                releases: github_releases
                    .into_iter()
                    .map(|release| CatalogRelease {
                        id: catalog_release_id(&compatibility_tool_flavor, release.id),
                        flavor: compatibility_tool_flavor.clone(),
                        release,
                    })
                    .collect(),
            }
        } else {
            Flavor {
                flavor: compatibility_tool_flavor,
                releases: Vec::new(),
            }
        }
    }

    async fn get_releases(
        &self,
        owner: &str,
        repository: &str,
        renew_cache: bool,
    ) -> Option<Vec<Release>> {
        const SECONDS_IN_A_DAY: u64 = 86_400;

        let path = env::var("DECKY_PLUGIN_RUNTIME_DIR").unwrap_or_else(|_| "/tmp/".to_string());

        let file_name = format!("github_releases_{}_{}_cache.json", owner, repository);
        let cache_file = PathBuf::from(path).join(&file_name);

        if !renew_cache && cache_file.exists() && cache_file.is_file() {
            match read_cached_releases(&cache_file) {
                Ok((modified, github_releases)) => {
                    let duration = SystemTime::now()
                        .duration_since(modified)
                        .unwrap_or_default();

                    if duration.as_secs() < SECONDS_IN_A_DAY {
                        self.app_state.lock().await.updater_last_check =
                            Some(unix_timestamp(modified));

                        if github_releases.is_empty() {
                            info!(
                                "Cached data is possibly corrupted or missing information from an older version. Renewing cache..."
                            );
                        } else {
                            return Some(github_releases);
                        }
                    } else {
                        info!("Cache file is older than 1 day. Fetching new releases.");
                    }
                }
                Err(err) => {
                    warn!(
                        "Failed to read cached releases from {}: {}",
                        cache_file.display(),
                        err
                    );
                }
            }
        }

        let github_releases = match github_util::list_all_releases(owner, repository).await {
            Ok(releases) => {
                if releases.is_empty() {
                    error!("No releases found.");
                    return None;
                }

                let current_time = SystemTime::now();
                self.app_state.lock().await.updater_last_check = Some(unix_timestamp(current_time));

                match serde_json::to_string(&releases) {
                    Ok(json) => {
                        if let Some(parent) = cache_file.parent() {
                            if let Err(err) = fs::create_dir_all(parent) {
                                warn!("Failed to prepare release cache directory: {}", err);
                            }
                        }
                        if let Err(err) = fs::write(&cache_file, json) {
                            warn!(
                                "Failed to write release cache {}: {}",
                                cache_file.display(),
                                err
                            );
                        }
                    }
                    Err(err) => warn!("Failed to serialize release cache: {}", err),
                }
                releases
            }
            Err(err) => {
                error!("{}", github_util::format_error_chain(&err));
                error!("full debug error: {err:#?}");

                if cache_file.exists() && cache_file.is_file() {
                    match read_cached_releases(&cache_file) {
                        Ok((modified, github_releases)) => {
                            self.app_state.lock().await.updater_last_check =
                                Some(unix_timestamp(modified));
                            warn!("Unable to fetch new releases. Using cached releases.");
                            github_releases
                        }
                        Err(cache_err) => {
                            error!(
                                "Unable to fetch new releases and cached releases are unusable: {}",
                                cache_err
                            );
                            return None;
                        }
                    }
                } else {
                    error!("Unable to fetch new releases. No cached releases found.");
                    return None;
                }
            }
        };

        Some(github_releases)
    }
}

fn read_cached_releases(cache_file: &Path) -> Result<(SystemTime, Vec<Release>), String> {
    let metadata =
        fs::metadata(cache_file).map_err(|err| format!("failed to read metadata: {}", err))?;
    let modified = metadata
        .modified()
        .map_err(|err| format!("failed to read modified timestamp: {}", err))?;
    let contents =
        fs::read_to_string(cache_file).map_err(|err| format!("failed to read file: {}", err))?;
    let releases = serde_json::from_str::<Vec<Release>>(&contents)
        .map_err(|err| format!("failed to parse JSON: {}", err))?;

    Ok((modified, releases))
}

fn unix_timestamp(time: SystemTime) -> u64 {
    time.duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

use crate::release_util::{self, Release, ReleaseProvider, ReleaseSource};
use crate::wine_cask::app::WineCask;
use crate::wine_cask::catalog::{FlavorDefinition, FLAVOR_DEFINITIONS};
use futures_util::future::join_all;
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
}

// SteamClient.Settings.GetGlobalCompatTools()
#[derive(Serialize, Deserialize, Clone)]
pub struct SteamClientCompatToolInfo {
    #[serde(rename = "strToolName")]
    pub str_tool_name: String,
    #[serde(rename = "strDisplayName")]
    pub str_display_name: String,
}

pub fn catalog_release_id(
    flavor: &CompatibilityToolFlavor,
    release_id: u64,
    source: ReleaseSource,
) -> String {
    match source {
        // Keep existing GitHub release IDs unchanged.
        ReleaseSource::GitHub => format!("catalog:{}:{}", flavor, release_id),
        ReleaseSource::Codeberg => format!("catalog:{}:codeberg:{}", flavor, release_id),
    }
}

impl WineCask {
    pub async fn get_flavors(&self, renew_cache: bool) -> Vec<Flavor> {
        let mut flavors = Vec::with_capacity(FLAVOR_DEFINITIONS.len());
        for definition in FLAVOR_DEFINITIONS {
            flavors.push(self.get_flavor(definition, renew_cache).await);
        }
        flavors
    }

    async fn get_flavor(&self, definition: &FlavorDefinition, renew_cache: bool) -> Flavor {
        let provider_releases = join_all(definition.providers.iter().map(|provider| async move {
            let releases = self
                .get_provider_releases(provider, renew_cache)
                .await
                .unwrap_or_default();
            (provider, releases)
        }))
        .await;
        definition.merge_releases(provider_releases)
    }

    async fn get_provider_releases(
        &self,
        provider: &ReleaseProvider,
        renew_cache: bool,
    ) -> Option<Vec<Release>> {
        const SECONDS_IN_A_DAY: u64 = 86_400;

        let path = env::var("DECKY_PLUGIN_RUNTIME_DIR").unwrap_or_else(|_| "/tmp/".to_string());

        let cache_file = PathBuf::from(path).join(provider.cache_file_name());

        if !renew_cache && cache_file.exists() && cache_file.is_file() {
            match read_cached_releases(&cache_file) {
                Ok((modified, releases)) => {
                    let duration = SystemTime::now()
                        .duration_since(modified)
                        .unwrap_or_default();

                    if duration.as_secs() < SECONDS_IN_A_DAY {
                        self.app_state.lock().await.updater_last_check =
                            Some(unix_timestamp(modified));

                        if releases.is_empty() {
                            info!(
                                "Cached data is possibly corrupted or missing information from an older version. Renewing cache..."
                            );
                        } else {
                            return Some(releases);
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

        let releases = match release_util::list_all_releases(provider).await {
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
                error!("{}", release_util::format_error_chain(&err));
                error!("full debug error: {err:#?}");

                if cache_file.exists() && cache_file.is_file() {
                    match read_cached_releases(&cache_file) {
                        Ok((modified, releases)) => {
                            self.app_state.lock().await.updater_last_check =
                                Some(unix_timestamp(modified));
                            warn!("Unable to fetch new releases. Using cached releases.");
                            releases
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

        Some(releases)
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

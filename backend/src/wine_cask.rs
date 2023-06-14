use std::fmt::Write;
use std::{env, fs, io};
use std::fs::File;
use std::io::{Read, Write as IoWrite};
use tokio::net::TcpStream;
use std::path::PathBuf;
use bytes::BytesMut;
use flate2::read::GzDecoder;
use ratchet_rs::{NoExt, PayloadType, WebSocket};
use serde::{Deserialize, Serialize};
use futures_util::StreamExt;
use log::{error, info};
use xz2::read::XzDecoder;
use crate::github_util;
use crate::github_util::{Asset, Release};
use crate::steam_util::{CompatibilityTool, SteamUtil};

// Internal only
#[derive(Serialize, Deserialize)]
pub struct VirtualCompatibilityToolMetadata {
    r#virtual: bool,
    virtual_original: String,
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

#[derive(Deserialize, Serialize, Clone)]
pub struct QueueCompatibilityTool {
    pub flavor: CompatibilityToolFlavor,
    pub name: String,
    pub url: String,
    pub state: QueueCompatibilityToolState,
    pub progress: u8,
}

#[derive(Deserialize, Serialize, PartialEq, Copy, Clone)]
pub enum QueueCompatibilityToolState {
    Extracting,
    Downloading,
    Waiting,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct AppState {
    pub available_flavors: Vec<Flavor>,
    pub installed_compatibility_tools: Vec<SteamCompatibilityTool>,
    pub in_progress: Option<QueueCompatibilityTool>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub enum CompatibilityToolFlavor {
    ProtonGE,
    SteamTinkerLaunch,
    Luxtorpeda,
    Boxtron,
}

#[derive(Serialize, Deserialize)]
pub struct Install {
    flavor: CompatibilityToolFlavor,
    install: Release,
    /*id: u64,
    tag_name: String,
    url: String,*/
}

#[derive(Serialize, Deserialize)]
pub struct Uninstall {
    flavor: CompatibilityToolFlavor,
    uninstall: SteamCompatibilityTool,
    /*internal_name: String,
    path: String,*/
}

#[derive(Serialize, Deserialize)]
pub struct Request {
    pub r#type: RequestType,
    pub app_state: Option<AppState>,
    pub install: Option<Install>,
    pub uninstall: Option<Uninstall>,
}

#[derive(Serialize, Deserialize, PartialEq)]
pub enum RequestType {
    RequestState,
    UpdateState,
    Install,
    Uninstall,
    Reboot,
    Notification,

    CreateVirtual,
    DeleteVirtual,
    UpdateVirtual,
}

pub struct WineCask {
    steam_util: SteamUtil,
}

impl WineCask {
    pub fn new() -> Self {
        if let Ok(decky_user_home) = std::env::var("DECKY_USER_HOME") {
            let steam_home: PathBuf = PathBuf::from(decky_user_home).join(".steam");
            if steam_home.exists() {
                let steam_util = SteamUtil::new(steam_home);
                return Self {
                    steam_util
                };
            } else {
                // todo: msg
            }
        } else {
            // todo: msg
        }
        if let Ok(steam_util) = SteamUtil::find() {
            Self {
                steam_util
            }
        } else {
            panic!("Something went wrong trying to use steam util!"); //fixme:
        }
    }

    pub async fn get_flavors(&self, installed_compatibility_tools: &Vec<SteamCompatibilityTool>, renew_cache: bool) -> Vec<Flavor> {
        let mut flavors = Vec::new();

        let proton_ge_flavor = self.get_flavor(installed_compatibility_tools.clone(), CompatibilityToolFlavor::ProtonGE, "GloriousEggroll", "proton-ge-custom", renew_cache).await; // fixme: has more than 100 releases
        let steam_tinker_launch_flavor = self.get_flavor(installed_compatibility_tools.clone(), CompatibilityToolFlavor::SteamTinkerLaunch, "sonic2kk", "steamtinkerlaunch", renew_cache).await;
        let luxtorpeda_flavor = self.get_flavor(installed_compatibility_tools.clone(), CompatibilityToolFlavor::Luxtorpeda, "luxtorpeda-dev", "luxtorpeda", renew_cache).await;
        let boxtron_flavor = self.get_flavor(installed_compatibility_tools.clone(), CompatibilityToolFlavor::Boxtron, "dreamer", "boxtron", renew_cache).await;

        flavors.push(proton_ge_flavor);
        flavors.push(steam_tinker_launch_flavor);
        flavors.push(luxtorpeda_flavor);
        flavors.push(boxtron_flavor);

        flavors
    }

    async fn get_flavor(&self, installed_compatibility_tools: Vec<SteamCompatibilityTool>, compatibility_tool_flavor: CompatibilityToolFlavor, owner: &str, repository: &str, renew_cache: bool) -> Flavor {
        let github_releases: Vec<Release> = self.get_releases(owner, repository, renew_cache).await.unwrap();

        let installed: Vec<SteamCompatibilityTool> = match compatibility_tool_flavor {
            CompatibilityToolFlavor::ProtonGE => {
                let installed = installed_compatibility_tools
                    .iter()
                    .filter(|x|
                        github_releases
                            .iter()
                            .any(|gh| x.internal_name == gh.tag_name || x.display_name == gh.tag_name)
                    ) //fixme: add directory_name
                    .cloned()
                    .collect();
                installed
            }
            CompatibilityToolFlavor::SteamTinkerLaunch => {
                let installed = installed_compatibility_tools
                    .iter()
                    .filter(|x|
                                github_releases
                                    .iter()
                                    .any(|gh| x.internal_name == "SteamTinkerLaunch".to_owned() + &gh.tag_name ||
                                        x.display_name == "Steam Tinker Launch ".to_owned() + &gh.tag_name) //fixme: add directory_name
                    )
                    .cloned()
                    .collect();
                installed
            }
            CompatibilityToolFlavor::Luxtorpeda => {
                let installed = installed_compatibility_tools
                    .iter()
                    .filter(|x|
                                github_releases
                                    .iter()
                                    .any(|gh| x.internal_name == "Luxtorpeda".to_owned() + &gh.tag_name ||
                                        x.display_name == "Luxtorpeda ".to_owned() + &gh.tag_name) //fixme: add directory_name
                    )
                    .cloned()
                    .collect();
                installed
            }
            CompatibilityToolFlavor::Boxtron => {
                let installed = installed_compatibility_tools
                    .iter()
                    .filter(|x|
                                github_releases
                                    .iter()
                                    .any(|gh| x.internal_name == "Boxtron".to_owned() + &gh.tag_name ||
                                        x.display_name == "Boxtron ".to_owned() + &gh.tag_name) //fixme: add directory_name
                    )
                    .cloned()
                    .collect();
                installed
            }
        };
        let not_installed: Vec<Release> = match compatibility_tool_flavor {
            CompatibilityToolFlavor::ProtonGE => {
                let not_installed = github_releases
                    .iter()
                    .filter(|x|
                                !installed_compatibility_tools
                                    .iter()
                                    .any(|ct| ct.internal_name == x.tag_name || ct.display_name == x.tag_name) //fixme: add directory_name
                    )
                    .cloned()
                    .collect();
                not_installed
            }
            CompatibilityToolFlavor::SteamTinkerLaunch => {
                let not_installed = github_releases
                    .iter()
                    .filter(|x|
                                !installed_compatibility_tools
                                    .iter()
                                    .any(|ct| ct.internal_name == "SteamTinkerLaunch".to_owned() + &x.tag_name ||
                                        ct.display_name == "Steam Tinker Launch ".to_owned() + &x.tag_name) //fixme: add directory_name
                    )
                    .cloned()
                    .collect();
                not_installed
            }
            CompatibilityToolFlavor::Luxtorpeda => {
                let not_installed = github_releases
                    .iter()
                    .filter(|x|
                                !installed_compatibility_tools
                                    .iter()
                                    .any(|ct| ct.internal_name == "Luxtorpeda".to_owned() + &x.tag_name ||
                                        ct.display_name == "Luxtorpeda ".to_owned() + &x.tag_name) //fixme: add directory_name
                    )
                    .cloned()
                    .collect();
                not_installed
            }
            CompatibilityToolFlavor::Boxtron => {
                let not_installed = github_releases
                    .iter()
                    .filter(|x|
                                !installed_compatibility_tools
                                    .iter()
                                    .any(|ct| ct.internal_name == "Boxtron".to_owned() + &x.tag_name ||
                                        ct.display_name == "Boxtron ".to_owned() + &x.tag_name) //fixme: add directory_name
                    )
                    .cloned()
                    .collect();
                not_installed
            }
        };
        Flavor {
            flavor: compatibility_tool_flavor,
            installed,
            not_installed,
        }
    }

    async fn get_releases(&self, owner: &str, repository: &str, renew_cache: bool) -> Option<Vec<Release>> { //fixme: error handling
        let path = env::var("DECKY_PLUGIN_RUNTIME_DIR").unwrap_or("/tmp/".parse().unwrap());

        let file_name = format!("github_releases_{}_{}_cache.json", owner, repository);
        let cache_file = PathBuf::from(path).join(file_name);

        if cache_file.exists() && cache_file.is_file() && !renew_cache {
            let string = fs::read_to_string(cache_file).unwrap();
            let github_releases: Vec<Release> = serde_json::from_str(&string).unwrap();
            Some(github_releases)
        } else {
            let github_releases: Vec<Release> = github_util::list_all_releases(owner, repository).await.unwrap();
            let json = serde_json::to_string(&github_releases).unwrap();
            fs::write(cache_file, &json).unwrap();
            Some(github_releases)
        }
    }

    pub fn get_used_by_games(&self, display_name: &str, internal_name: &str) -> Vec<String> {
        let compat_tools_mapping = self.steam_util.get_compatibility_tools_mappings().expect("Failed to get compatibility tools mappings");
        let installed_games = self.steam_util.list_installed_games().expect("Failed to get list of installed games");
        let used_by_games: Vec<String> = installed_games
            .iter()
            .filter(|game| compat_tools_mapping.contains_key(&game.app_id) &&
                (compat_tools_mapping.get(&game.app_id).unwrap().eq(display_name)
                    || compat_tools_mapping.get(&game.app_id).unwrap().eq(internal_name))
            )
            .map(|game| game.name.clone())
            .collect();
        return used_by_games;
    }

    pub fn update_used_by_games(&self, app_state: &mut AppState) {
        for compat_tool in &mut app_state.installed_compatibility_tools {
            compat_tool.used_by_games = self.get_used_by_games(&compat_tool.display_name, &compat_tool.internal_name);
        }
    }

    pub fn list_compatibility_tools(&self) -> Option<Vec<SteamCompatibilityTool>> {
        let compat_tools = self.steam_util.list_compatibility_tools().expect("Failed to get list of compatibility tools");

        let mut compatibility_tools: Vec<SteamCompatibilityTool> = Vec::new();

        for compat_tool in &compat_tools {
            let used_by_games: Vec<String> = self.get_used_by_games(&compat_tool.display_name, &compat_tool.internal_name);
            let metadata = self.lookup_virtual_compatibility_tool_metadata(&compat_tool);
            compatibility_tools.push(SteamCompatibilityTool {
                path: compat_tool.path.to_str().unwrap().to_string(),
                //directory_name: compat_tool.directory_name.to_string(),
                display_name: compat_tool.display_name.to_string(),
                internal_name: compat_tool.internal_name.to_string(),
                used_by_games,
                requires_restart: false,
                //r#virtual: metadata.r#virtual,
                //virtual_original: metadata.virtual_original,
            })
        }

        Some(compatibility_tools)
    }

    fn lookup_virtual_compatibility_tool_metadata(&self, compat_tool: &CompatibilityTool) -> VirtualCompatibilityToolMetadata {
        let metadata_file = compat_tool.path.join("wine-cask-metadata.json"); // fixme: Store in runtime data dir instead
        return if metadata_file.exists() && metadata_file.is_file() {
            let metadata = fs::read_to_string(metadata_file).unwrap();
            let metadata: VirtualCompatibilityToolMetadata = serde_json::from_str(&metadata).unwrap();
            metadata
        } else {
            let metadata = VirtualCompatibilityToolMetadata {
                r#virtual: false,
                virtual_original: "".to_string(),
            };
            metadata
        };
    }

    fn create_virtual_compatibility_tool(&self, name: &str, virtual_original_path: PathBuf) {
        let path = self.steam_util.get_steam_compatibility_tools_directory().join(name);
        if path.exists() {
            // todo: already exist
        }

        fs::create_dir(&path).expect("TODO: panic message");
        fs::copy(virtual_original_path, &path).expect("TODO: panic message");

        // Generate virtual compat tool vdf
        let compat_tool_vdf_path = path.join("compatibilitytool.vdf");
        let virtual_original = self.steam_util.read_compatibility_tool_from_vdf_path(&compat_tool_vdf_path).unwrap().display_name;
        self.generate_compatibility_tool_vdf(compat_tool_vdf_path, &name.replace(" ", "-"), name);

        // Create virtual compat tool metadata
        let metadata_file = path.join("wine-cask-metadata.json");
        let metadata = VirtualCompatibilityToolMetadata {
            r#virtual: true,
            virtual_original,
        };
        fs::write(metadata_file, serde_json::to_string_pretty(&metadata).unwrap()).unwrap();
    }

    fn generate_compatibility_tool_vdf(&self, path: PathBuf, internal_name: &str, display_name: &str) {
        let mut file = File::create(path).expect("Failed to create file");
        writeln!(file, r#""compatibilitytools"
            {{
              "compat_tools"
              {{
                "{}"
                {{
                  "install_path" "."
                  "display_name" "{}"
                  "from_oslist"  "windows"
                  "to_oslist"    "linux"
                }}
              }}
            }}"#, internal_name, display_name).expect("Failed to write to file");
    }

    pub async fn install_compatibility_tool(&self, install_request: Install, app_state: &mut AppState, websocket: &mut WebSocket<TcpStream, NoExt>) {
        let _rename_compat_tool_vdf: bool = match install_request.flavor {
            CompatibilityToolFlavor::ProtonGE => {
                false
            }
            CompatibilityToolFlavor::SteamTinkerLaunch => {
                true
            }
            CompatibilityToolFlavor::Luxtorpeda => {
                true
            }
            CompatibilityToolFlavor::Boxtron => {
                true
            }
        };

        if let Some(mut queue_compat_tool) = look_for_compressed_archive(&install_request) {
            // Mark as downloading
            queue_compat_tool.state = QueueCompatibilityToolState::Downloading;
            queue_compat_tool.progress = 0;
            app_state.in_progress = Some(queue_compat_tool.clone());
            websocket_update_state(app_state.clone(), websocket).await;
            //serialize and sent to websocket

            let client = reqwest::Client::new();
            let response_wrapped = client.get(&queue_compat_tool.url).send().await;
            let response = response_wrapped.unwrap();

            let total_size = response.content_length().unwrap_or(0);

            let mut downloaded_bytes = Vec::new();
            let mut downloaded_size = 0;

            let mut body = response.bytes_stream();
            while let Some(chunk_result) = body.next().await {
                let chunk = chunk_result.unwrap();
                downloaded_bytes.extend_from_slice(&chunk);
                downloaded_size += chunk.len() as u64;

                let progress = ((downloaded_size.clone() as f64 / total_size.clone() as f64) * 100.0) as u8;
                if queue_compat_tool.progress != progress { // we send an update for every percent instead of time
                    queue_compat_tool.progress = progress;
                    app_state.in_progress = Some(queue_compat_tool.clone());
                    websocket_update_state(app_state.clone(), websocket).await;
                }
            }
            //Mark
            queue_compat_tool.state = QueueCompatibilityToolState::Extracting;
            queue_compat_tool.progress = 0;
            app_state.in_progress = Some(queue_compat_tool.clone());
            websocket_update_state(app_state.clone(), websocket).await;
            //serialize and sent to websocket

            let reader = io::Cursor::new(downloaded_bytes); // fixme: probably save this to runtime dir
            let decompressed: Box<dyn Read> = if queue_compat_tool.url.ends_with(".tar.gz") {
                Box::new(GzDecoder::new(reader))
            } else {
                Box::new(XzDecoder::new(reader))
            };
            let mut tar = tar::Archive::new(decompressed);
            tar.unpack(self.steam_util.get_steam_compatibility_tools_directory()).unwrap();
            /*let entries = tar.entries().unwrap();
            let mut extracted_bytes = 0;
            let total_bytes = entries.to
                .map(|entry| entry.unwrap().header().size().unwrap())
                .sum::<u64>();

            for entry in tar.entries().unwrap() { //cannot call entries unless archive is at \ position 0
                let mut entry = entry.unwrap();
                let entry_path = entry.path().unwrap();
                let dest_path = format!("{}/{}", self.steam_util.get_steam_compatibility_tools_directory().to_str().unwrap().to_string(), entry_path.display());

                if let Some(parent) = entry_path.parent() {
                    fs::create_dir_all(&parent).unwrap();
                }

                if entry_path.is_dir() {
                    println!("Entry Dir Path: {}", entry_path.to_str().unwrap());
                    fs::create_dir_all(&dest_path).unwrap();
                } else {
                    let mut dest_file = File::create(&dest_path).unwrap();
                    let mut buffer = Vec::new();
                    entry.read_to_end(&mut buffer).unwrap();
                    dest_file.write_all(&buffer).unwrap();
                }
                extracted_bytes += entry.header().size().unwrap();

                let progress = ((extracted_bytes.clone() as f64 / total_bytes.clone() as f64) * 100.0) as u8;
                if queue_compat_tool.progress != progress { // we send an update for every percent instead of time
                    queue_compat_tool.progress = progress;
                    app_state.in_progress = Some(queue_compat_tool.clone());
                    websocket_update_state(app_state.clone(), websocket).await;
                }
            }*/
            queue_compat_tool.progress = 100;
            app_state.in_progress = Some(queue_compat_tool.clone());
            websocket_update_state(app_state.clone(), websocket).await;

            //fixme: terrible workaround
            let new_installed_compat_tools = self.find_unlisted_directories(&app_state.installed_compatibility_tools);
            let first = new_installed_compat_tools.get(0).unwrap();

            let new_compat_tool_vdf = first.path.join("compatibilitytool.vdf");
            let new_path = match queue_compat_tool.flavor {
                CompatibilityToolFlavor::ProtonGE => {
                    first.path.to_owned()
                }
                CompatibilityToolFlavor::SteamTinkerLaunch => {
                    self.generate_compatibility_tool_vdf(new_compat_tool_vdf, &format!("SteamTinkerLaunch{}", &install_request.install.tag_name), &format!("Steam Tinker Launch {}", &install_request.install.tag_name));
                    self.steam_util.get_steam_compatibility_tools_directory().join(&format!("SteamTinkerLaunch{}", &install_request.install.tag_name))
                }
                CompatibilityToolFlavor::Luxtorpeda => {
                    self.generate_compatibility_tool_vdf(new_compat_tool_vdf, &format!("Luxtorpeda{}", &install_request.install.tag_name), &format!("Luxtorpeda {}", &install_request.install.tag_name));
                    self.steam_util.get_steam_compatibility_tools_directory().join(&format!("Luxtorpeda{}", &install_request.install.tag_name))
                }
                CompatibilityToolFlavor::Boxtron => {
                    self.generate_compatibility_tool_vdf(new_compat_tool_vdf, &format!("Boxtron{}", &install_request.install.tag_name), &format!("Boxtron {}", &install_request.install.tag_name));
                    self.steam_util.get_steam_compatibility_tools_directory().join(&format!("Boxtron{}", &install_request.install.tag_name))
                }
            };
            fs::rename(&first.path, &new_path).unwrap();
            let new_installed_compat_tools = self.find_unlisted_directories(&app_state.installed_compatibility_tools);
            let first = new_installed_compat_tools.get(0).unwrap();
            app_state.installed_compatibility_tools.push(SteamCompatibilityTool {
                path: new_path.to_str().unwrap().to_string(),
                //directory_name: queue_compat_tool.to_owned().directory_name,
                internal_name: first.internal_name.to_string(),
                display_name: first.display_name.to_string(),
                requires_restart: true,
                used_by_games: vec![],
                //r#virtual: false,
                //virtual_original: "".to_string(),
            });
            app_state.in_progress = None;
            app_state.available_flavors = self.get_flavors(&app_state.installed_compatibility_tools, false).await;
            websocket_update_state(app_state.clone(), websocket).await;
            //websocket_notification("Successfully installed ".to_owned() + &unboxed_request.name, websocket).await;
        } else {
            //todo: steamtinkerlaunch tarball
            // todo: oh no something went wrong
        }
    }

    pub async fn uninstall_compatibility_tool(&self, uninstall_request: Uninstall, app_state: &mut AppState, websocket: &mut WebSocket<TcpStream, NoExt>) {
        let directory_path = PathBuf::from(&uninstall_request.uninstall.path);
        recursive_delete_dir_entry(&directory_path).expect("TODO: panic message");
        if let Some(index) = app_state.installed_compatibility_tools.iter().position(|x| x.internal_name == uninstall_request.uninstall.internal_name && x.path == uninstall_request.uninstall.path) {
            app_state.installed_compatibility_tools.remove(index);
        }
        app_state.available_flavors = self.get_flavors(&app_state.installed_compatibility_tools, false).await;
        websocket_update_state(app_state.clone(), websocket).await;
        //websocket_notification("Successfully uninstalled ".to_owned() + name, websocket).await;
    }

    fn find_unlisted_directories(&self, installed_compatibility_tools: &Vec<SteamCompatibilityTool>) -> Vec<CompatibilityTool>{
        self.steam_util.list_compatibility_tools().unwrap().iter().filter(|refresh|
            !installed_compatibility_tools
                .iter()
                .any(|ct| refresh.internal_name == ct.internal_name)
        ).cloned().collect()
    }
}

fn recursive_delete_dir_entry(entry_path: &PathBuf) -> io::Result<()> {
    if entry_path.is_dir() {
        for entry in fs::read_dir(entry_path)? {
            let entry = entry?;
            let path = entry.path();
            recursive_delete_dir_entry(&path)?;
        }
        fs::remove_dir(entry_path)?;
    } else {
        fs::remove_file(entry_path)?;
    }

    Ok(())
}

pub async fn websocket_update_state(app_state: AppState, websocket: &mut WebSocket<TcpStream, NoExt>) {
    let mut buf_new = BytesMut::new();
    let response_new: Request = Request {
        r#type: RequestType::UpdateState,
        app_state: Some(app_state),
        install: None,
        uninstall: None,
    };
    let update = serde_json::to_string(&response_new).unwrap();
    info!("Websocket message sent: {}", update);
    buf_new.write_str(&update).expect("TODO: panic message");
    if websocket.is_active() {
        websocket.write(buf_new, PayloadType::Text).await.expect("TODO: panic message");
    } else {
        error!("Websocket connection isn't alive! Failed to update state");
    }
}

pub fn look_for_compressed_archive(install_request: &Install) -> Option<QueueCompatibilityTool> {
    /*if install_request.flavor == CompatibilityToolFlavor::SteamTinkerLaunch {// fixme: doesn't actually work
        return Some(QueueCompatibilityTool {
            flavor: install_request.flavor.to_owned(),
            name: install_request.install.tag_name.to_owned(),
            url: install_request.install.tarball_url.to_owned(),
            state: QueueCompatibilityToolState::Extracting,
            progress: 0,
        });
    }*/


    let is_compressed = |asset: &Asset| {
        asset.content_type == "application/gzip" || asset.content_type == "application/x-xz"
    };

    if let Some(asset) = install_request.install.assets.clone().into_iter().find(is_compressed) {
        return Some(QueueCompatibilityTool {
            flavor: install_request.flavor.to_owned(),
            name: install_request.install.tag_name.to_owned(),
            url: asset.browser_download_url.to_owned(),
            state: QueueCompatibilityToolState::Extracting,
            progress: 0,
        });
    }

    None
}
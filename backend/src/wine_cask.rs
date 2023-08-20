use crate::steam_util::{CompatibilityTool, SteamUtil};
use crate::{github_util, PeerMap};
use flate2::read::GzDecoder;
use log::{error, info};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fs::File;
use std::io::Read;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use std::{env, fs, io};
use xz2::read::XzDecoder;

use crate::github_util::{Asset, Release};
use futures_util::StreamExt;
use tokio::sync::Mutex;
use tokio_tungstenite::tungstenite::Message;

// Internal only
#[derive(Serialize, Deserialize, Clone)]
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

#[derive(Deserialize, Serialize, PartialEq, Clone)]
pub enum QueueCompatibilityToolState {
    Extracting,
    Downloading,
    Waiting,
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub enum CompatibilityToolFlavor {
    ProtonGE,
    SteamTinkerLaunch,
    Luxtorpeda,
    Boxtron,
}

impl std::fmt::Display for CompatibilityToolFlavor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CompatibilityToolFlavor::ProtonGE => write!(f, "ProtonGE"),
            CompatibilityToolFlavor::SteamTinkerLaunch => write!(f, "SteamTinkerLaunch"),
            CompatibilityToolFlavor::Luxtorpeda => write!(f, "Luxtorpeda"),
            CompatibilityToolFlavor::Boxtron => write!(f, "Boxtron"),
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Install {
    flavor: CompatibilityToolFlavor,
    install: Release,
    /*id: u64,
    tag_name: String,
    url: String,*/
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Uninstall {
    pub flavor: CompatibilityToolFlavor,
    pub uninstall: SteamCompatibilityTool,
    /*internal_name: String,
    path: String,*/
}

#[derive(Serialize, Deserialize, PartialEq, Clone)]
pub enum RequestType {
    RequestState,
    UpdateState,
    Install,
    Uninstall,
    Reboot,
    Notification,
    Task,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Task {
    pub r#type: TaskType,
    pub uninstall: Option<SteamCompatibilityTool>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub enum TaskType {
    InstallCompatibilityTool,
    UninstallCompatibilityTool,
    Reboot,

    //
    CreateVirtual,
    DeleteVirtual,
    UpdateVirtual,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct AppState {
    pub available_flavors: Vec<Flavor>,
    pub installed_compatibility_tools: Vec<SteamCompatibilityTool>,
    pub in_progress: Option<QueueCompatibilityTool>,
    pub queue: VecDeque<Install>,
    pub task_queue: VecDeque<Task>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Request {
    pub r#type: RequestType,
    pub app_state: Option<AppState>,
    pub install: Option<Install>,
    pub uninstall: Option<Uninstall>,
}

pub struct WineCask {
    pub steam_util: SteamUtil,
    pub app_state: Arc<Mutex<AppState>>,
}

impl WineCask {
    async fn task_queue_pop_front(&self) -> Option<Task> {
        self.app_state.lock().await.task_queue.pop_front()
    }

    pub async fn add_to_task_queue(&self, task: Task) {
        self.app_state.lock().await.task_queue.push_back(task);
    }

    async fn queue_pop_front(&self) -> Option<Install> {
        self.app_state.lock().await.queue.pop_front()
    }

    pub async fn add_to_queue(&self, queue_compatibility: Install) {
        self.app_state.lock().await.queue.push_back(queue_compatibility);

        //todo: verify if already in queue
    }

    pub async fn remove_from_queue(&self, install: Install) {
        let mut app_state = self.app_state.lock().await;
        if let Some(position) = app_state
            .queue
            .iter()
            .position(|x| x.install.url == install.install.url)
        {
            app_state.queue.remove(position);
            //Todo: Notify removed from queue
        } else {
            //Todo: Notify none found
        }
    }

    async fn install_compatibility_tool(&self, install: Install, peer_map: &PeerMap) {
        if let Some(mut queue_compatibility_tool) = look_for_compressed_archive(&install) {
            // Mark as downloading...
            {
                queue_compatibility_tool.state = QueueCompatibilityToolState::Downloading;
                queue_compatibility_tool.progress = 0;
                self.app_state.lock().await.in_progress = Some(queue_compatibility_tool.clone());
                self.broadcast_app_state(&peer_map).await;
            }

            let client = reqwest::Client::new();
            let response_wrapped = client.get(&queue_compatibility_tool.url).send().await;
            let response = response_wrapped.unwrap();
            let total_size = response.content_length().unwrap_or(0);
            let mut downloaded_bytes = Vec::new();
            let mut downloaded_size = 0;
            let mut body = response.bytes_stream();
            while let Some(chunk_result) = body.next().await { // fixme: we need to timeout when internet connection is lost while downloading...
                let chunk = chunk_result.unwrap();
                downloaded_bytes.extend_from_slice(&chunk);
                downloaded_size += chunk.len() as u64;

                let progress = ((downloaded_size as f64 / total_size as f64) * 100.0) as u8;
                if queue_compatibility_tool.progress != progress {
                    // we send an update for every percent instead of time
                    queue_compatibility_tool.progress = progress;
                    // Update progress...
                    {
                        self.app_state.lock().await.in_progress =
                            Some(queue_compatibility_tool.clone());
                        self.broadcast_app_state(&peer_map).await;
                    }
                }
            }
            let reader = io::Cursor::new(downloaded_bytes); // fixme: probably save this to runtime dir
            // Mark as extracting...
            {
                queue_compatibility_tool.state = QueueCompatibilityToolState::Extracting;
                queue_compatibility_tool.progress = 0;
                self.app_state.lock().await.in_progress = Some(queue_compatibility_tool.clone());
                self.broadcast_app_state(&peer_map).await;
            }

            // Spawn a new thread for the extraction process
            // Why do we need this turns out unpack process is blocking, because of this async function doesn't yield control back to Rust runtime until the extraction is finished.
            let directory = self
                .steam_util
                .get_steam_compatibility_tools_directory()
                .clone();
            let queue_compatibility_tool_clone = queue_compatibility_tool.clone(); // Clone the queue_compatibility_tool

            tokio::task::spawn_blocking(move || {
                let decompressed: Box<dyn Read> =
                    if queue_compatibility_tool_clone.url.ends_with(".tar.gz") {
                        Box::new(GzDecoder::new(reader))
                    } else {
                        Box::new(XzDecoder::new(reader))
                    };
                let mut tar = tar::Archive::new(decompressed);
                tar.unpack(directory).unwrap();
            })
                .await
                .unwrap();

            // Mark as completed
            {
                queue_compatibility_tool.progress = 100;
                self.app_state.lock().await.in_progress = Some(queue_compatibility_tool.clone());
                self.broadcast_app_state(&peer_map).await;
            }
            //fixme: terrible workaround
            /*let new_installed_compat_tools = self.find_unlisted_directories(&self.app_state.lock().await.installed_compatibility_tools);
            let first = new_installed_compat_tools.get(0).unwrap();

            let new_compat_tool_vdf = first.path.join("compatibilitytool.vdf");
            let new_path = match queue_compat_tool.flavor {
                CompatibilityToolFlavor::ProtonGE => {
                    first.path.to_owned()
                }
                CompatibilityToolFlavor::SteamTinkerLaunch => {
                    self.generate_compatibility_tool_vdf(new_compat_tool_vdf, &format!("SteamTinkerLaunch{}", &install_request.install.tag_name), &format!("Steam Tinker Launch {}", &install_request.install.tag_name));
                    self.steam_util.get_steam_compatibility_tools_directory().join(format!("SteamTinkerLaunch{}", &install_request.install.tag_name))
                }
                CompatibilityToolFlavor::Luxtorpeda => {
                    self.generate_compatibility_tool_vdf(new_compat_tool_vdf, &format!("Luxtorpeda{}", &install_request.install.tag_name), &format!("Luxtorpeda {}", &install_request.install.tag_name));
                    self.steam_util.get_steam_compatibility_tools_directory().join(format!("Luxtorpeda{}", &install_request.install.tag_name))
                }
                CompatibilityToolFlavor::Boxtron => {
                    self.generate_compatibility_tool_vdf(new_compat_tool_vdf, &format!("Boxtron{}", &install_request.install.tag_name), &format!("Boxtron {}", &install_request.install.tag_name));
                    self.steam_util.get_steam_compatibility_tools_directory().join(format!("Boxtron{}", &install_request.install.tag_name))
                }
            };
            fs::rename(&first.path, &new_path).unwrap();
            let new_installed_compat_tools = self.find_unlisted_directories(&self.app_state.lock().await.installed_compatibility_tools);
            let first = new_installed_compat_tools.get(0).unwrap();*/
            {
                let mut app_state = self.app_state.lock().await;
                app_state
                    .installed_compatibility_tools
                    .push(SteamCompatibilityTool {
                        path: "".to_string(),
                        internal_name: queue_compatibility_tool.name.to_string(),
                        display_name: queue_compatibility_tool.name.to_string(),
                        requires_restart: true,
                        used_by_games: vec![],
                    });
                app_state.in_progress = None;
            }
            let installed = self
                .app_state
                .lock()
                .await
                .installed_compatibility_tools
                .clone();
            self.app_state.lock().await.available_flavors =
                self.get_flavors(installed, false).await;
            self.broadcast_app_state(&peer_map).await;
        } else {}
    }

    //fixme: AppState queue to task rather than Install[]
    pub async fn uninstall_compatibility_tool(&self, steam_compatibility_tool: SteamCompatibilityTool, peer_map: &PeerMap) {
        let directory_path = PathBuf::from(&steam_compatibility_tool.path);
        recursive_delete_dir_entry(&directory_path).expect("TODO: panic message");
        if let Some(index) = {
            let app_state = self.app_state.lock().await;
            app_state
                .installed_compatibility_tools
                .iter()
                .position(|x| {
                    x.internal_name == steam_compatibility_tool.internal_name
                        && x.path == steam_compatibility_tool.path
                })
        }
        {
            self.app_state.lock().await.installed_compatibility_tools.remove(index);
        }
        let installed = self
            .app_state
            .lock()
            .await
            .installed_compatibility_tools
            .clone();
        self.app_state.lock().await.available_flavors =
            self.get_flavors(installed, false).await;
        self.broadcast_app_state(&peer_map).await;
    }

    pub async fn broadcast_app_state(&self, peer_map: &PeerMap) {
        let app_state = self.app_state.lock().await;
        let response_new: Request = Request {
            r#type: RequestType::UpdateState,
            app_state: Some(app_state.clone()),
            install: None,
            uninstall: None,
        };
        let update = serde_json::to_string(&response_new).unwrap();
        let message = Message::text(update);
        for recp in peer_map.lock().await.values() {
            match recp.unbounded_send(message.clone()) {
                Ok(_) => {
                    //info!("Websocket message sent: {}", update);
                    println!("Sent message to recipient");
                }
                Err(e) => {
                    error!("Failed to send websocket message: {}", e);
                    println!("Failed to send message to recipient");
                }
            }
        }
    }

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
        let steam_tinker_launch_flavor = self
            .get_flavor(
                &installed_compatibility_tools,
                CompatibilityToolFlavor::SteamTinkerLaunch,
                "sonic2kk",
                "steamtinkerlaunch",
                renew_cache,
            )
            .await;
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
        flavors.push(steam_tinker_launch_flavor);
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
                            tool.internal_name
                                == compatibility_tool_flavor.to_string() + " " + &gh.tag_name
                                || tool.display_name
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
                            tool.internal_name
                                == compatibility_tool_flavor.to_string() + " " + &gh.tag_name
                                || tool.display_name
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
        //fixme: error handling
        let path = env::var("DECKY_PLUGIN_RUNTIME_DIR").unwrap_or("/tmp/".parse().unwrap());

        let file_name = format!("github_releases_{}_{}_cache.json", owner, repository);
        let cache_file = PathBuf::from(path).join(file_name);

        let mut renew_cache = renew_cache;

        if cache_file.exists() && cache_file.is_file() && !renew_cache {
            let metadata = fs::metadata(&cache_file).unwrap();
            let modified = metadata.modified().unwrap();

            // Calculate the duration between the current time and the file modification time
            let now = SystemTime::now();
            let duration = now.duration_since(modified).unwrap_or(Duration::from_secs(0));

            // Check if the cache file is a day or more old (86400 seconds in a day)
            if duration.as_secs() < 86400 {
                let string = fs::read_to_string(&cache_file).unwrap();
                let github_releases: Vec<Release> = serde_json::from_str(&string).unwrap();
                return Some(github_releases);
            } else {
                // Cache file is too old, renew the cache
                println!("Cache is too old. Renewing cache...");
                renew_cache = true;
            }
        } else {
            renew_cache = true;
        }

        if renew_cache {
            let github_releases: Vec<Release> = github_util::list_all_releases(owner, repository)
                .await
                .unwrap();
            let json = serde_json::to_string(&github_releases).unwrap();
            fs::write(cache_file, json).unwrap();
            Some(github_releases)
        } else {
            None
        }
    }

    fn get_used_by_games(&self, display_name: &str, internal_name: &str) -> Vec<String> {
        let compat_tools_mapping = self
            .steam_util
            .get_compatibility_tools_mappings()
            .expect("Failed to get compatibility tools mappings");
        let installed_games = self
            .steam_util
            .list_installed_games()
            .expect("Failed to get list of installed games");
        let used_by_games: Vec<String> = installed_games
            .iter()
            .filter(|game| {
                compat_tools_mapping.contains_key(&game.app_id)
                    && (compat_tools_mapping
                    .get(&game.app_id)
                    .unwrap()
                    .eq(display_name)
                    || compat_tools_mapping
                    .get(&game.app_id)
                    .unwrap()
                    .eq(internal_name))
            })
            .map(|game| game.name.clone())
            .collect();
        used_by_games
    }

    pub async fn update_used_by_games(&self) {
        for compat_tool in &mut self.app_state.lock().await.installed_compatibility_tools {
            compat_tool.used_by_games =
                self.get_used_by_games(&compat_tool.display_name, &compat_tool.internal_name);
        }
    }

    pub fn list_compatibility_tools(&self) -> Option<Vec<SteamCompatibilityTool>> {
        let compat_tools = self
            .steam_util
            .list_compatibility_tools()
            .expect("Failed to get list of compatibility tools");

        let mut compatibility_tools: Vec<SteamCompatibilityTool> = Vec::new();

        for compat_tool in &compat_tools {
            let used_by_games: Vec<String> =
                self.get_used_by_games(&compat_tool.display_name, &compat_tool.internal_name);
            //let metadata = self.lookup_virtual_compatibility_tool_metadata(compat_tool);
            compatibility_tools.push(SteamCompatibilityTool {
                path: compat_tool.path.to_string_lossy().to_string(),
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

    fn find_unlisted_directories(&self, installed_compatibility_tools: &Vec<SteamCompatibilityTool>) -> Vec<CompatibilityTool> {
        self.steam_util
            .list_compatibility_tools()
            .unwrap()
            .iter()
            .filter(|refresh| {
                !installed_compatibility_tools
                    .iter()
                    .any(|ct| refresh.internal_name == ct.internal_name)
            })
            .cloned()
            .collect()
    }

    fn lookup_virtual_compatibility_tool_metadata(&self, compat_tool: &CompatibilityTool) -> VirtualCompatibilityToolMetadata {
        let metadata_file = compat_tool.path.join("wine-cask-metadata.json"); // fixme: Store in runtime data dir instead
        if metadata_file.exists() && metadata_file.is_file() {
            let metadata = fs::read_to_string(metadata_file).unwrap();
            let metadata: VirtualCompatibilityToolMetadata =
                serde_json::from_str(&metadata).unwrap();
            metadata
        } else {
            VirtualCompatibilityToolMetadata {
                r#virtual: false,
                virtual_original: "".to_string(),
            }
        }
    }

    fn create_virtual_compatibility_tool(&self, name: &str, virtual_original_path: PathBuf) {
        let path = self
            .steam_util
            .get_steam_compatibility_tools_directory()
            .join(name);
        if path.exists() {
            // todo: already exist
        }

        fs::create_dir(&path).expect("TODO: panic message");
        fs::copy(virtual_original_path, &path).expect("TODO: panic message");

        // Generate virtual compat tool vdf
        let compat_tool_vdf_path = path.join("compatibilitytool.vdf");
        let virtual_original = self
            .steam_util
            .read_compatibility_tool_from_vdf_path(&compat_tool_vdf_path)
            .unwrap()
            .display_name;
        self.generate_compatibility_tool_vdf(compat_tool_vdf_path, &name.replace(' ', "-"), name);

        // Create virtual compat tool metadata
        let metadata_file = path.join("wine-cask-metadata.json");
        let metadata = VirtualCompatibilityToolMetadata {
            r#virtual: true,
            virtual_original,
        };
        fs::write(
            metadata_file,
            serde_json::to_string_pretty(&metadata).unwrap(),
        )
            .unwrap();
    }

    fn generate_compatibility_tool_vdf(&self, path: PathBuf, internal_name: &str, display_name: &str) {
        let mut file = File::create(path).expect("Failed to create file");
        writeln!(
            file,
            r#""compatibilitytools"
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
            }}"#,
            internal_name, display_name
        )
            .expect("Failed to write to file");
    }
}

fn recursive_delete_dir_entry(entry_path: &std::path::Path) -> io::Result<()> {
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

    if let Some(asset) = install_request
        .install
        .assets
        .clone()
        .into_iter()
        .find(is_compressed)
    {
        return Some(QueueCompatibilityTool {
            flavor: install_request.flavor.to_owned(),
            name: install_request.install.tag_name.to_owned(),
            url: asset.browser_download_url,
            state: QueueCompatibilityToolState::Extracting,
            progress: 0,
        });
    }

    None
}

pub async fn process_queue(wine_cask: Arc<WineCask>, peer_map: PeerMap) {
    loop {
        match wine_cask.task_queue_pop_front().await {
            Some(task) => {
                if task.r#type == TaskType::UninstallCompatibilityTool {
                    wine_cask
                        .uninstall_compatibility_tool(task.uninstall.unwrap(), &peer_map)
                        .await;
                } else if task.r#type == TaskType::Reboot {
                    wine_cask
                        .app_state
                        .lock()
                        .await
                        .installed_compatibility_tools =
                        wine_cask.list_compatibility_tools().unwrap();
                    let installed = wine_cask
                        .app_state
                        .lock()
                        .await
                        .installed_compatibility_tools
                        .to_owned();
                    wine_cask.app_state.lock().await.available_flavors =
                        wine_cask.get_flavors(installed, true).await;
                    wine_cask.broadcast_app_state(&peer_map).await;
                }
            }
            None => {}
        }
        match wine_cask.queue_pop_front().await {
            Some(install) => {
                wine_cask
                    .install_compatibility_tool(install, &peer_map)
                    .await;
            }
            None => {}
        };
    }
}
use std::fmt::Write;
use std::{fs, io};
use std::io::Read;
use tokio::net::TcpStream;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use bytes::BytesMut;
use flate2::read::GzDecoder;
use ratchet_rs::{NoExt, PayloadType, WebSocket};
use serde::{Deserialize, Serialize};
use futures_util::StreamExt;
use log::{error, info};
use xz2::read::XzDecoder;
use crate::steam_util::SteamUtil;

#[derive(Serialize, Deserialize, Clone)]
pub struct SteamCompatibilityTool {
    pub name: String,
    pub display_name: String,
    pub internal_name: String,
    pub used_by_games: Vec<String>,
    pub requires_restart: bool,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct QueueCompatibilityTool {
    pub flavor: CompatibilityToolFlavor,
    pub name: String,
    pub url: String,
    pub state: QueueCompatibilityToolState,
    pub progress: i32,
}

#[derive(Deserialize, Serialize, PartialEq, Copy, Clone)]
pub enum QueueCompatibilityToolState {
    Extracting,
    Downloading,
    Waiting,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct AppState {
    pub installed_compatibility_tools: Vec<SteamCompatibilityTool>,
    pub in_progress: Option<QueueCompatibilityTool>
}

#[derive(Serialize, Deserialize, Clone)]
pub enum CompatibilityToolFlavor {
    ProtonGE,
    SteamTinkerLaunch,
    Luxtorpeda,
    Boxtron
}

#[derive(Serialize, Deserialize)]
pub struct Install {
    flavor: CompatibilityToolFlavor,
    url: String,
}

#[derive(Serialize, Deserialize)]
pub struct Uninstall {
    flavor: CompatibilityToolFlavor,
    name: String,
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

    pub fn get_used_by_games(&self, name: &str, display_name: &str, internal_name: &str) -> Vec<String> {
        let compat_tools_mapping = self.steam_util.get_compatibility_tools_mappings().expect("Failed to get compatibility tools mappings");
        let installed_games = self.steam_util.list_installed_games().expect("Failed to get list of installed games");
        let used_by_games: Vec<String> = installed_games
            .iter()
            .filter(|game| compat_tools_mapping.contains_key(&game.app_id) &&
                (compat_tools_mapping.get(&game.app_id).unwrap().eq(name)
                    || compat_tools_mapping.get(&game.app_id).unwrap().eq(display_name)
                    || compat_tools_mapping.get(&game.app_id).unwrap().eq(internal_name))
            )
            .map(|game| game.name.clone())
            .collect();
        return used_by_games;
    }

    pub fn update_used_by_games(&self, app_state: &mut AppState) {
        for compat_tool in &mut app_state.installed_compatibility_tools {
            compat_tool.used_by_games = self.get_used_by_games(&compat_tool.name, &compat_tool.display_name, &compat_tool.internal_name);
        }
    }

    pub fn list_compatibility_tools(&self) -> Option<Vec<SteamCompatibilityTool>> {
        let compat_tools = self.steam_util.list_compatibility_tools().expect("Failed to get list of compatibility tools");

        let mut compatibility_tools: Vec<SteamCompatibilityTool> = Vec::new();

        for compat_tool in &compat_tools {
            let used_by_games: Vec<String> = self.get_used_by_games(&compat_tool.name, &compat_tool.display_name, &compat_tool.internal_name);
            compatibility_tools.push(SteamCompatibilityTool {
                name: compat_tool.name.to_string(),
                display_name: compat_tool.display_name.to_string(),
                internal_name: compat_tool.internal_name.to_string(),
                used_by_games,
                requires_restart: false,
            })
        }

        Some(compatibility_tools)
    }

    pub async fn install_compatibility_tool(&self, install_request: Install, app_state: &mut AppState, websocket: &mut WebSocket<TcpStream, NoExt>) {
        let rename_compat_tool_vdf: bool = match install_request.flavor {
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

        if let Some(mut queue_compat_tool) = github_release_assets_lookup(install_request).await {
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
            let mut start_time = Instant::now();
            while let Some(chunk_result) = body.next().await {
                let chunk = chunk_result.unwrap();
                downloaded_bytes.extend_from_slice(&chunk);
                downloaded_size += chunk.len() as u64;

                queue_compat_tool.progress = ((downloaded_size as f64 / total_size as f64) * 100.0) as i32;
                if start_time.elapsed() >= Duration::from_millis(250) {
                    app_state.in_progress = Some(queue_compat_tool.clone());
                    websocket_update_state(app_state.clone(), websocket).await;
                    start_time = Instant::now();
                }
            }
            //Mark
            queue_compat_tool.state = QueueCompatibilityToolState::Extracting;
            queue_compat_tool.progress = 0;
            app_state.in_progress = Some(queue_compat_tool.clone());
            websocket_update_state(app_state.clone(), websocket).await;
            //serialize and sent to websocket

            let reader = std::io::Cursor::new(downloaded_bytes); // fixme: probably save this to runtime dir
            // fixme: only does tar.gz
            let decompressed: Box<dyn Read> = if queue_compat_tool.url.ends_with(".tar.gz") {
                Box::new(GzDecoder::new(reader))
            } else {
                Box::new(XzDecoder::new(reader))
            };
            let mut tar = tar::Archive::new(decompressed);
            tar.unpack(self.steam_util.get_steam_compatibility_tools_directory()).unwrap();
            queue_compat_tool.progress = 100; // Fixme: no progress for extracting
            app_state.in_progress = Some(queue_compat_tool.clone());
            websocket_update_state(app_state.clone(), websocket).await;

            app_state.installed_compatibility_tools.push(SteamCompatibilityTool {
                name: queue_compat_tool.to_owned().name,
                internal_name: queue_compat_tool.to_owned().name,
                display_name: queue_compat_tool.to_owned().name,
                requires_restart: true,
                used_by_games: vec![],
            });
            app_state.in_progress = None;
            websocket_update_state(app_state.clone(), websocket).await;
            //websocket_notification("Successfully installed ".to_owned() + &unboxed_request.name, websocket).await;
        } else {
            // todo: oh no something went wrong
        }
    }

    pub async fn uninstall_compatibility_tool(&self, uninstall_request: Uninstall, app_state: &mut AppState, websocket: &mut WebSocket<TcpStream, NoExt>) {
        //fixme: will not work for anything else other than GE
        if let Ok(entries) = std::fs::read_dir(self.steam_util.get_steam_compatibility_tools_directory()) {
            for entry in entries {
                if let Ok(entry) = entry {
                    if entry.file_name().to_str().unwrap().eq(&uninstall_request.name) {
                        let path = entry.path().as_path().to_owned();
                        recursive_delete_dir_entry(&path).expect("TODO: panic message");
                        break;
                    }
                }
            }
        }
        if let Some(index) = app_state.installed_compatibility_tools.iter().position(|x| x.name == uninstall_request.name || x.internal_name == uninstall_request.name || x.display_name == uninstall_request.name) {
            app_state.installed_compatibility_tools.remove(index);
        }
        websocket_update_state(app_state.clone(), websocket).await;
        //websocket_notification("Successfully uninstalled ".to_owned() + name, websocket).await;
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

pub async fn github_release_assets_lookup(install_request: Install) -> Option<QueueCompatibilityTool> { // fixme: Only Steam Tinker Launch won't work with this. we just need to tarball it and find dir rename it
    // todo: we need to be able to tell the frontend that there is not internet available and we can't download anything
    let client = reqwest::Client::builder()
        .user_agent("FlashyReese/decky-wine-cellar")
        .build()
        .expect("Failed to create HTTP client");
    let response = client.get(&install_request.url).send().await.expect("Failed to fetch JSON").text().await.unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&response).unwrap();

    if let Some(release) = parsed.as_object() {
        if let Some(assets) = release.get("assets").and_then(|a| a.as_array()) {
            let mut url_zip = String::new();

            for asset in assets {
                if let Some(content_type) = asset.get("content_type").and_then(|ct| ct.as_str()) {
                    if content_type == "application/gzip" || content_type == "application/x-xz" {
                        if let Some(download_url) = asset.get("browser_download_url").and_then(|url| url.as_str()) {
                            url_zip = download_url.to_string();
                        }
                        break;
                    }
                }
            }

            if url_zip.is_empty() {
                //fixme: println!("No ZIP content found in {}", release.get("tag_name").unwrap_or(&Value::Null));
                return None;
            }
            return Some(QueueCompatibilityTool {
                flavor: install_request.flavor,
                name: release.get("tag_name").unwrap_or(&serde_json::Value::Null).as_str().unwrap().to_string(),
                url: url_zip,
                state: QueueCompatibilityToolState::Waiting,
                progress: 0,
            });
        }
    }
    None
}
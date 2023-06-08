use std::fmt::Write;
use std::{env, fs, io};
use std::path::PathBuf;
use std::time::{Duration, Instant};
use bytes::BytesMut;
use flate2::read::GzDecoder;
use ratchet_rs::{NoExt, PayloadType, WebSocket};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::net::TcpStream;
use crate::steam::SteamCompatibilityTool;
use futures_util::StreamExt;
use log::info;

#[derive(Deserialize, Serialize, Clone)]
pub struct Response {
    pub(crate) r#type: ResponseType,
    pub(crate) message: Option<String>,
    pub(crate) url: Option<String>,
    pub(crate) name: Option<String>,
    pub(crate) installed: Option<Vec<SteamCompatibilityTool>>,
    pub(crate) in_progress: Option<QueueCompatibilityTool>,
}

#[derive(Deserialize, Serialize, PartialEq, Clone)]
pub enum ResponseType {
    Install,
    Uninstall,
    RequestState,
    UpdateState,
    Notification
}

#[derive(Deserialize, Serialize, Clone)]
pub struct QueueCompatibilityTool {
    pub(crate) name: String,
    pub(crate) url: String,
    pub(crate) state: QueueCompatibilityToolState,
    pub(crate) progress: i32,
}

#[derive(Deserialize, Serialize, PartialEq, Copy, Clone)]
pub enum QueueCompatibilityToolState {
    Extracting,
    Downloading,
    Waiting,
}

pub async fn websocket_notification(message: String, websocket: &mut WebSocket<TcpStream, NoExt>) {
    let mut buf_new = BytesMut::new();
    let response_new: Response = Response {
        r#type: ResponseType::Notification,
        message: Some(message),
        url: None,
        name: None,
        installed: None,
        in_progress: None,
    };
    let update = serde_json::to_string(&response_new).unwrap();
    info!("Websocket message sent: {}", update);
    buf_new.write_str(&update).expect("TODO: panic message");
    websocket.write(buf_new, PayloadType::Text).await.expect("TODO: panic message");
}

pub async fn websocket_update_state(internal_installed: Vec<SteamCompatibilityTool>, queue: Option<QueueCompatibilityTool>, websocket: &mut WebSocket<TcpStream, NoExt>) {
    let mut buf_new = BytesMut::new();
    let response_new: Response = Response {
        r#type: ResponseType::UpdateState,
        message: None,
        url: None,
        name: None,
        installed: Some(internal_installed),
        in_progress: queue,
    };
    let update = serde_json::to_string(&response_new).unwrap();
    info!("Websocket message sent: {}", update);
    buf_new.write_str(&update).expect("TODO: panic message");
    websocket.write(buf_new, PayloadType::Text).await.expect("TODO: panic message");
}

pub async fn install_compatibility_tool(compatibility_tools_path: &PathBuf, response: &Response, internal_installed: &mut Vec<SteamCompatibilityTool>, websocket: &mut WebSocket<TcpStream, NoExt>) {
    let queue = github_gzip_lookup(response.url.clone().unwrap()).await;
    if let Some(mut unboxed_request) = queue {
        // Mark as downloading
        unboxed_request.state = QueueCompatibilityToolState::Downloading;
        unboxed_request.progress = 0;
        websocket_update_state(internal_installed.clone(), Some(unboxed_request.clone()), websocket).await;
        //serialize and sent to websocket

        let client = Client::new();
        let response_wrapped = client.get(&unboxed_request.url).send().await;
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

            unboxed_request.progress = ((downloaded_size as f64 / total_size as f64) * 100.0) as i32;
            if start_time.elapsed() >= Duration::from_millis(250) {
                websocket_update_state(internal_installed.clone(), Some(unboxed_request.clone()), websocket).await;
                start_time = Instant::now();
            }
        }
        //Mark
        unboxed_request.state = QueueCompatibilityToolState::Extracting;
        unboxed_request.progress = 0;
        websocket_update_state(internal_installed.clone(), Some(unboxed_request.clone()), websocket).await;
        //serialize and sent to websocket

        let reader = std::io::Cursor::new(downloaded_bytes); // fixme: probably save this to runtime dir
        // fixme: only does tar.gz
        let decompressed = GzDecoder::new(reader);
        let mut tar = tar::Archive::new(decompressed);
        tar.unpack(compatibility_tools_path.to_str().unwrap()).unwrap();
        unboxed_request.progress = 100; // Fixme: no progress for extracting
        websocket_update_state(internal_installed.clone(), Some(unboxed_request.clone()), websocket).await;

        internal_installed.push(SteamCompatibilityTool {
            name: unboxed_request.to_owned().name,
            internal_name: unboxed_request.to_owned().name,
            display_name: unboxed_request.to_owned().name,
            version: None,
            path: compatibility_tools_path.join(unboxed_request.to_owned().name).to_str().unwrap().parse().unwrap(),
            requires_restart: true,
        });
        websocket_update_state(internal_installed.clone(), None, websocket).await;
        websocket_notification("Successfully installed ".to_owned() + &unboxed_request.name, websocket).await;
    }
}

pub async fn uninstall_compatibility_tool(compatibility_tools_path: &PathBuf, name: &str, internal_installed: &mut Vec<SteamCompatibilityTool>, websocket: &mut WebSocket<TcpStream, NoExt>) {
    if let Ok(entries) = std::fs::read_dir(compatibility_tools_path) {
        for entry in entries {
            if let Ok(entry) = entry {
                if entry.file_name().to_str().unwrap().eq(name) {
                    let path = entry.path().as_path().to_owned();
                    recursive_delete_dir_entry(&path).expect("TODO: panic message");
                    break;
                }
            }
        }
    }
    if let Some(index) = internal_installed.iter().position(|x| x.name == name || x.internal_name == name || x.display_name == name) {
        internal_installed.remove(index);
    }
    websocket_update_state(internal_installed.clone(), None, websocket).await;
    websocket_notification("Successfully uninstalled ".to_owned() + name, websocket).await;
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

pub async fn github_gzip_lookup(url: String) -> Option<QueueCompatibilityTool> {
    let client = Client::builder()
        .user_agent("Rust")
        .build()
        .expect("Failed to create HTTP client");
    let response = client.get(url).send().await.expect("Failed to fetch JSON").text().await.unwrap();
    let parsed: Value = serde_json::from_str(&response).unwrap();

    if let Some(release) = parsed.as_object() {
        if let Some(assets) = release.get("assets").and_then(|a| a.as_array()) {
            let mut url_zip = String::new();

            for asset in assets {
                if let Some(content_type) = asset.get("content_type").and_then(|ct| ct.as_str()) {
                    if content_type == "application/gzip" {
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
                name: release.get("tag_name").unwrap_or(&Value::Null).as_str().unwrap().to_string(),
                url: url_zip,
                state: QueueCompatibilityToolState::Waiting,
                progress: 0,
            });
        }
    }
    None
}

pub fn print_all_env() {
    for env in env::vars() {
        println!("{} = {}", env.0, env.1);
    }
}
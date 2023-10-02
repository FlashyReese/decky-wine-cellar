mod github_util;
mod multilogger;
mod steam_util;
mod wine_cask;

use crate::multilogger::MultiLogger;
use crate::steam_util::SteamUtil;
use crate::wine_cask::app::{AppState, Request, RequestType, TaskType, UpdaterState, WineCask};
use futures_channel::mpsc::{unbounded, UnboundedSender};
use futures_util::{future, pin_mut, stream::TryStreamExt, StreamExt};
use log::{error, info, Level};
use std::collections::{HashMap, VecDeque};
use std::env;
use std::fs::OpenOptions;
use std::io::Error as IoError;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use tokio_tungstenite::tungstenite::Message;

type Tx = UnboundedSender<Message>;
type PeerMap = Arc<Mutex<HashMap<SocketAddr, Tx>>>;
type AsyncAppState = Arc<Mutex<AppState>>;
type ArcWineCask = Arc<WineCask>;

#[tokio::main]
async fn main() -> Result<(), IoError> {
    configure_logger().unwrap();

    let addr = get_server_address();

    let state = PeerMap::new(Mutex::new(HashMap::new()));

    let steam_util = SteamUtil::new(get_steam_directory());

    let app_state = AsyncAppState::new(Mutex::new(AppState {
        available_flavors: Vec::new(),
        installed_compatibility_tools: Vec::new(),
        installed_applications: Vec::new(),
        in_progress: None,
        task_queue: VecDeque::new(),
        updater_state: UpdaterState::Idle,
        updater_last_check: None,
        available_compat_tools: None,
        flavors: Vec::new(),
    }));

    let wine_cask = WineCask {
        steam_util,
        app_state: app_state.clone(),
    };

    initialize_app_state(&wine_cask).await;

    let wine_cask_arc = ArcWineCask::new(wine_cask);

    tokio::spawn(wine_cask::process_queue(
        wine_cask_arc.clone(),
        state.clone(),
    ));

    start_server(addr, wine_cask_arc, state).await;

    info!("Exiting...");
    Ok(())
}

async fn start_server(addr: String, wine_cask: Arc<WineCask>, state: PeerMap) {
    let try_socket = TcpListener::bind(&addr).await;
    let listener = try_socket.expect("Failed to bind");
    info!("Listening on: {}", addr);

    while let Ok((stream, addr)) = listener.accept().await {
        tokio::spawn(handle_connection(
            wine_cask.clone(),
            state.clone(),
            stream,
            addr,
        ));
    }
}

async fn handle_connection(
    wine_cask: Arc<WineCask>,
    peer_map: PeerMap,
    raw_stream: TcpStream,
    addr: SocketAddr,
) {
    info!("Incoming TCP connection from: {}", addr);

    let ws_stream = tokio_tungstenite::accept_async(raw_stream)
        .await
        .expect("Error during the websocket handshake occurred");
    info!("WebSocket connection established: {}", addr);

    let (tx, rx) = unbounded();
    peer_map.lock().await.insert(addr, tx);

    let (outgoing, incoming) = ws_stream.split();

    let broadcast_incoming = incoming.try_for_each_concurrent(Some(10), |msg| {
        let wine_cask_clone = Arc::clone(&wine_cask);
        let peer_map_clone = Arc::clone(&peer_map);
        async move {
            if msg.is_text() {
                info!(
                    "Received a message from {}: {}",
                    addr,
                    msg.to_text().unwrap()
                );

                if let Ok(msg) = &msg.to_text() {
                    if !msg.is_empty() {
                        handle_request(&wine_cask_clone, msg, &peer_map_clone).await;
                    }
                }
            } else {
                info!("Unhandled message from {}: {:?}", addr, msg);
            }

            Ok(())
        }
    });

    let receive_from_others = rx.map(Ok).forward(outgoing);

    pin_mut!(broadcast_incoming, receive_from_others);
    future::select(broadcast_incoming, receive_from_others).await;

    info!("{} disconnected", &addr);
    peer_map.lock().await.remove(&addr);
}

fn configure_logger() -> Result<(), IoError> {
    // Check for DECKY_PLUGIN_LOG environment variable
    let log_path = match env::var("DECKY_PLUGIN_LOG") {
        Ok(path) => path,
        Err(_) => {
            // If DECKY_PLUGIN_LOG is not found, check for DECKY_PLUGIN_LOG_DIR
            match env::var("DECKY_PLUGIN_LOG_DIR") {
                Ok(log_dir) => {
                    // Create the log directory if it doesn't exist
                    format!("{}/wine-cask.log", log_dir)
                }
                Err(_) => {
                    // If neither variable is set, use the /tmp directory
                    "/tmp/decky-wine-cellar.log".to_string()
                }
            }
        }
    };

    let target = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)?;

    MultiLogger::init(target, Level::Info).expect("Could not configure logger");

    info!("Logging to: {}", log_path);

    Ok(())
}

fn get_server_address() -> String {
    env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:8887".to_string())
}

fn get_steam_directory() -> PathBuf {
    match env::var("DECKY_USER_HOME") {
        Ok(value) => {
            info!("Using DECKY_USER_HOME: {}", value);
            SteamUtil::find_steam_directory(Some(value)).unwrap() // Todo: Handle if no steam folder is found, although this should never happen
        }
        Err(_) => {
            error!(
                "Couldn't find environment variable DECKY_USER_HOME, using default steam directory"
            );
            SteamUtil::find_steam_directory(None).unwrap()
        }
    }
}

async fn initialize_app_state(wine_cask: &WineCask) {
    let mut app_state = wine_cask.app_state.lock().await;
    app_state.installed_compatibility_tools = wine_cask.list_compatibility_tools().unwrap();
    app_state.installed_applications = wine_cask.list_installed_applications();
}

async fn handle_request(wine_cask: &Arc<WineCask>, msg: &str, peer_map: &PeerMap) {
    if let Ok(request) = serde_json::from_str::<Request>(msg) {
        match request.r#type {
            RequestType::RequestState => {
                // Assumes available_compat_tools is Some
                wine_cask
                    .process_frontend_compat_tools_update(
                        peer_map,
                        request.available_compat_tools.unwrap(),
                    )
                    .await;
                wine_cask.update_used_by_games(peer_map).await;
            }
            RequestType::Task => {
                if let Some(task) = request.task {
                    if task.r#type == TaskType::InstallCompatibilityTool {
                        wine_cask.add_to_task_queue(task, peer_map).await;
                    } else if task.r#type == TaskType::CancelCompatibilityToolInstall {
                        wine_cask
                            .remove_or_cancel_from_task_queue(task, peer_map)
                            .await;
                    } else if task.r#type == TaskType::UninstallCompatibilityTool {
                        wine_cask
                            .uninstall_compatibility_tool(
                                task.uninstall.unwrap().steam_compatibility_tool,
                                peer_map,
                            )
                            .await;
                    } else if task.r#type == TaskType::CheckForFlavorUpdates {
                        wine_cask.check_for_flavor_updates(peer_map, true).await;
                    }
                } else {
                    wine_cask
                        .broadcast_notification(
                            peer_map,
                            "Error: Something went wrong with the task request",
                        )
                        .await;
                }
            }
            _ => {}
        }
    }
}

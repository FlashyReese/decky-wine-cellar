mod github_util;
mod multilogger;
mod steam_util;
mod wine_cask;

use crate::multilogger::MultiLogger;
use crate::steam_util::SteamUtil;
use crate::wine_cask::app::{
    AppState, Command, MessageEnvelope, MessageType, UpdaterState, WineCask,
};
use futures_channel::mpsc::{unbounded, UnboundedSender};
use futures_util::{future, pin_mut, stream::TryStreamExt, StreamExt};
use log::{error, info, warn, Level};
use std::collections::{HashMap, VecDeque};
use std::env;
use std::fs::OpenOptions;
use std::io::{Error as IoError, ErrorKind};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{Mutex, Notify};
use tokio_tungstenite::tungstenite::{
    handshake::server::{ErrorResponse, Request, Response},
    http::{header::ORIGIN, StatusCode},
    Message,
};

const STEAM_LOOPBACK_ORIGIN: &[u8] = b"https://steamloopback.host";

type Tx = UnboundedSender<Message>;
type PeerMap = Arc<Mutex<HashMap<SocketAddr, Tx>>>;
type AsyncAppState = Arc<Mutex<AppState>>;
type ArcWineCask = Arc<WineCask>;

#[tokio::main]
async fn main() -> Result<(), IoError> {
    configure_logger()?;

    let addr = get_server_address();

    let state = PeerMap::new(Mutex::new(HashMap::new()));
    let queue_notify = Arc::new(Notify::new());

    let steam_util = SteamUtil::new(get_steam_directory()?);
    let app_state = AsyncAppState::new(Mutex::new(AppState {
        catalog_flavors: Vec::new(),
        installed_tools: Vec::new(),
        virtual_tools: Vec::new(),
        app_compat_tool_mappings: HashMap::new(),
        app_compat_tool_mappings_stale: true,
        current_operation: None,
        queued_operations: Vec::new(),
        updater_state: UpdaterState::Idle,
        updater_last_check: None,
        steam_visible_tools: Vec::new(),
        operation_queue: VecDeque::new(),
        app_compat_tool_mappings_last_refresh_attempt: None,
        app_compat_tool_mappings_refresh_in_progress: false,
    }));

    let wine_cask = WineCask {
        steam_util,
        app_state,
        operation_broadcast_cache: Arc::new(Mutex::new(None)),
        queue_notify: queue_notify.clone(),
        virtual_tool_manifest_path: get_virtual_tool_manifest_path(),
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
    let listener = match TcpListener::bind(&addr).await {
        Ok(listener) => listener,
        Err(err) => {
            error!("Failed to bind websocket server to {}: {}", addr, err);
            return;
        }
    };
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

    let ws_stream =
        match tokio_tungstenite::accept_hdr_async(raw_stream, validate_websocket_origin).await {
            Ok(stream) => stream,
            Err(err) => {
                warn!(
                    "WebSocket handshake rejected or failed for {}: {}",
                    addr, err
                );
                return;
            }
        };
    info!("WebSocket connection established: {}", addr);

    let (tx, rx) = unbounded();
    peer_map.lock().await.insert(addr, tx);

    let (outgoing, incoming) = ws_stream.split();

    let broadcast_incoming = incoming.try_for_each_concurrent(Some(10), |msg| {
        let wine_cask_clone = Arc::clone(&wine_cask);
        let peer_map_clone = Arc::clone(&peer_map);
        async move {
            if msg.is_text() {
                match msg.to_text() {
                    Ok(text) if !text.is_empty() => {
                        info!("Received a message from {}", addr);
                        handle_request(&wine_cask_clone, text, &peer_map_clone).await;
                    }
                    Ok(_) => warn!("Received empty websocket message from {}", addr),
                    Err(err) => warn!("Received invalid text frame from {}: {}", addr, err),
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

fn validate_websocket_origin(
    request: &Request,
    response: Response,
) -> Result<Response, ErrorResponse> {
    let mut origins = request.headers().get_all(ORIGIN).iter();
    let origin_is_allowed = origins
        .next()
        .is_some_and(|origin| origin.as_bytes() == STEAM_LOOPBACK_ORIGIN)
        && origins.next().is_none();

    if origin_is_allowed {
        return Ok(response);
    }

    let mut rejection = ErrorResponse::new(Some("Forbidden".to_string()));
    *rejection.status_mut() = StatusCode::FORBIDDEN;
    Err(rejection)
}

fn configure_logger() -> Result<(), IoError> {
    let log_path = match env::var("DECKY_PLUGIN_LOG") {
        Ok(path) => path,
        Err(_) => match env::var("DECKY_PLUGIN_LOG_DIR") {
            Ok(log_dir) => format!("{}/wine-cask.log", log_dir),
            Err(_) => "/tmp/decky-wine-cellar.log".to_string(),
        },
    };

    let target = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)?;

    MultiLogger::init(target, Level::Info)
        .map_err(|err| IoError::other(format!("Could not configure logger: {err}")))?;

    info!("Logging to: {}", log_path);

    Ok(())
}

fn get_server_address() -> String {
    env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:8887".to_string())
}

fn get_steam_directory() -> Result<PathBuf, IoError> {
    let result = match env::var("DECKY_USER_HOME") {
        Ok(value) => {
            info!("Using DECKY_USER_HOME: {}", value);
            SteamUtil::find_steam_directory(Some(value))
        }
        Err(_) => {
            warn!("Couldn't find DECKY_USER_HOME, trying HOME/USERPROFILE defaults");
            SteamUtil::find_steam_directory(None)
        }
    };

    result.map_err(|err| IoError::new(ErrorKind::NotFound, err.to_string()))
}

fn get_virtual_tool_manifest_path() -> PathBuf {
    let base_dir = env::var("DECKY_PLUGIN_SETTINGS_DIR")
        .or_else(|_| env::var("DECKY_PLUGIN_RUNTIME_DIR"))
        .unwrap_or_else(|_| "/tmp/decky-wine-cellar".to_string());
    PathBuf::from(base_dir).join("virtual_tools.json")
}

async fn initialize_app_state(wine_cask: &WineCask) {
    wine_cask.sync_backend_state().await;
}

async fn handle_request(wine_cask: &Arc<WineCask>, msg: &str, peer_map: &PeerMap) {
    let request = match serde_json::from_str::<MessageEnvelope>(msg) {
        Ok(request) => request,
        Err(err) => {
            warn!("Failed to parse websocket request: {}", err);
            wine_cask
                .broadcast_notification(peer_map, "Error: Invalid request payload")
                .await;
            return;
        }
    };

    match request.r#type {
        MessageType::GetState => {
            wine_cask.refresh_app_compat_tool_mappings().await;
            wine_cask.broadcast_app_state(peer_map).await;
        }
        MessageType::ReportSteamVisibleTools => {
            if let Some(steam_visible_tools) = request.steam_visible_tools {
                wine_cask
                    .process_frontend_compat_tools_update(peer_map, steam_visible_tools)
                    .await;
            } else {
                wine_cask
                    .broadcast_notification(
                        peer_map,
                        "Error: Steam tool observation payload missing",
                    )
                    .await;
            }
        }
        MessageType::Command => {
            if let Some(command) = request.command {
                match command {
                    Command::RefreshCatalog => {
                        wine_cask.check_for_flavor_updates(peer_map, true).await;
                    }
                    Command::InstallCatalogRelease { release_id, target } => {
                        wine_cask
                            .queue_install_catalog_release(release_id, target, peer_map)
                            .await;
                    }
                    Command::UninstallInstalledTool { installed_tool_id } => {
                        wine_cask
                            .queue_uninstall_installed_tool(installed_tool_id, peer_map)
                            .await;
                    }
                    Command::CancelOperation { operation_id } => {
                        wine_cask.cancel_operation(operation_id, peer_map).await;
                    }
                    Command::CreateVirtualTool { user_label } => {
                        wine_cask
                            .queue_create_virtual_tool(user_label, peer_map)
                            .await;
                    }
                    Command::RenameVirtualTool {
                        virtual_tool_id,
                        user_label,
                    } => {
                        wine_cask
                            .queue_rename_virtual_tool(virtual_tool_id, user_label, peer_map)
                            .await;
                    }
                    Command::RemoveVirtualTool { virtual_tool_id } => {
                        wine_cask
                            .queue_remove_virtual_tool(virtual_tool_id, peer_map)
                            .await;
                    }
                }
            } else {
                wine_cask
                    .broadcast_notification(
                        peer_map,
                        "Error: Command request missing command payload",
                    )
                    .await;
            }
        }
        MessageType::UpdateState | MessageType::UpdateOperations | MessageType::Notification => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request_with_origin(origin: Option<&str>) -> Request {
        let mut request = Request::new(());
        if let Some(origin) = origin {
            request
                .headers_mut()
                .insert(ORIGIN, origin.parse().expect("test Origin must be valid"));
        }
        request
    }

    #[test]
    fn accepts_the_steam_loopback_origin() {
        let request = request_with_origin(Some("https://steamloopback.host"));

        assert!(validate_websocket_origin(&request, Response::new(())).is_ok());
    }

    #[test]
    fn rejects_a_different_origin() {
        let request = request_with_origin(Some("https://example.com"));

        let rejection = validate_websocket_origin(&request, Response::new(()))
            .expect_err("a foreign Origin must be rejected");
        assert_eq!(rejection.status(), StatusCode::FORBIDDEN);
    }

    #[test]
    fn rejects_a_missing_origin() {
        let request = request_with_origin(None);

        let rejection = validate_websocket_origin(&request, Response::new(()))
            .expect_err("a missing Origin must be rejected");
        assert_eq!(rejection.status(), StatusCode::FORBIDDEN);
    }

    #[test]
    fn rejects_origin_lookalikes_and_default_port_variants() {
        for origin in [
            "https://steamloopback.host.example.com",
            "https://steamloopback.host:443",
            "http://steamloopback.host",
        ] {
            let request = request_with_origin(Some(origin));
            assert!(validate_websocket_origin(&request, Response::new(())).is_err());
        }
    }

    #[test]
    fn rejects_multiple_origin_headers() {
        let mut request = request_with_origin(Some("https://steamloopback.host"));
        request.headers_mut().append(
            ORIGIN,
            "https://example.com"
                .parse()
                .expect("test Origin must be valid"),
        );

        assert!(validate_websocket_origin(&request, Response::new(())).is_err());
    }
}

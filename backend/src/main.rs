mod github_util;
mod steam_util;
mod wine_cask;

use async_tungstenite::tungstenite::protocol::Message;
use futures_channel::mpsc::{unbounded, UnboundedSender};
use futures_util::{future, pin_mut, stream::TryStreamExt, StreamExt};
use log::{info, LevelFilter};
use std::collections::{HashMap, VecDeque};
use std::env;
use std::fs::OpenOptions;
use std::io::{Error as IoError, Write as IoWrite};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use env_logger::Env;
use smol::net::{TcpListener, TcpStream};
use crate::steam_util::SteamUtil;
use crate::wine_cask::{AppState, Request, RequestType, Task, TaskType, WineCask};

type Tx = UnboundedSender<Message>;
type PeerMap = Arc<Mutex<HashMap<SocketAddr, Tx>>>;
type AsyncAppState = Arc<Mutex<AppState>>;
type ArcWineCask = Arc<WineCask>;

#[tokio::main]
async fn main() -> Result<(), IoError> {
    configure_logger()?;

    let addr = get_server_address();

    let state = PeerMap::new(Mutex::new(HashMap::new()));

    let steam_util = SteamUtil::new(get_steam_directory());

    let app_state = AsyncAppState::new(Mutex::new(AppState {
        available_flavors: Vec::new(),
        installed_compatibility_tools: Vec::new(),
        in_progress: None,
        queue: VecDeque::new(),
        task_queue: VecDeque::new(),
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
    println!("Listening on: {}", addr);

    while let Ok((stream, addr)) = listener.accept().await {
        tokio::spawn(handle_connection(
            wine_cask.clone(),
            state.clone(),
            stream,
            addr,
        ));
    }
}

async fn handle_connection(wine_cask: Arc<WineCask>, peer_map: PeerMap, raw_stream: TcpStream, addr: SocketAddr) {
    println!("Incoming TCP connection from: {}", addr);

    let ws_stream = async_tungstenite::accept_async(raw_stream)
        .await
        .expect("Error during the websocket handshake occurred");
    println!("WebSocket connection established: {}", addr);

    let (tx, rx) = unbounded();
    peer_map.lock().unwrap().insert(addr, tx);

    let (outgoing, incoming) = ws_stream.split();

    let broadcast_incoming = incoming.try_for_each(|msg| {
        if msg.is_text() {
            println!(
                "Received a message from {}: {}",
                addr,
                msg.to_text().unwrap()
            );

            if let Ok(msg) = &msg.to_text() {
                if !msg.is_empty() {
                    handle_request(&wine_cask, msg, &peer_map);
                }
            }
        } else {
            println!("Unhandled message from {}: {:?}", addr, msg);
        }

        future::ok(())
    });

    let receive_from_others = rx.map(Ok).forward(outgoing);

    pin_mut!(broadcast_incoming, receive_from_others);
    tokio::select! {
        _ = broadcast_incoming => {},
        _ = receive_from_others => {},
    }

    println!("{} disconnected", &addr);
    peer_map.lock().unwrap().remove(&addr);
}

fn configure_logger() -> Result<(), IoError> {
    let path = env::var("DECKY_PLUGIN_LOG")
        .unwrap_or_else(|_| "/tmp/decky-wine-cellar.log".to_string());

    let target = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;

    env_logger::Builder::from_env(Env::default().default_filter_or("info"))
        .format(|buf, record| {
            writeln!(
                buf,
                "[Wine Cask] {} {} {}",
                chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                record.level(),
                record.args()
            )
        })
        .filter(None, LevelFilter::Info)
        .target(env_logger::Target::Pipe(Box::new(target)))
        .init();

    Ok(())
}

fn get_server_address() -> String {
    env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:8887".to_string())
}

fn get_steam_directory() -> PathBuf {
    PathBuf::from(env::var("DECKY_USER_HOME").unwrap()).join(".steam")
}

async fn initialize_app_state(wine_cask: &WineCask) {
    let mut appstate = wine_cask.app_state.lock().unwrap();
    appstate.installed_compatibility_tools = wine_cask.list_compatibility_tools().unwrap();
    appstate.available_flavors = wine_cask
        .get_flavors(appstate.installed_compatibility_tools.clone(), false)
        .await;
}

fn handle_request(wine_cask: &Arc<WineCask>, msg: &str, peer_map: &PeerMap) {
    if let Ok(request) = serde_json::from_str::<Request>(&msg) {
        match request.r#type {
            RequestType::RequestState => {
                wine_cask.update_used_by_games();
                wine_cask.broadcast_app_state(&peer_map);
            }
            RequestType::Install => {
                wine_cask.add_to_queue(request.install.unwrap());
            }
            RequestType::Uninstall => {
                let uninstall = request.uninstall.unwrap().uninstall;
                wine_cask.add_to_task_queue(Task {
                    r#type: TaskType::UninstallCompatibilityTool,
                    uninstall: Some(uninstall),
                })
            }
            RequestType::Reboot => {
                wine_cask.add_to_task_queue(Task {
                    r#type: TaskType::Reboot,
                    uninstall: None,
                })
            }
            _ => {}
        }
    }
}
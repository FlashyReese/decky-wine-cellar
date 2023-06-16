mod steam_util;
mod wine_cask;
mod github_util;

use env_logger::Env;

/*
fixme: potential issues,
existing installs of steamtinkerlaunch, luxtorpeda, boxtron will not be detected proper in return break our installer function
solution: is to extract in a tmp directory generate our vdf then copy to our desired dir instead of extracting directly to compat tools dir and renaming.

 steamtinkerlaunch tarballs not return proper files: it redirects to https://codeload.github.com/sonic2kk/steamtinkerlaunch/legacy.tar.gz/refs/tags/v12.12


 */

use std::{
    io::Write as IoWrite,
    collections::HashMap,
    env,
    io::Error as IoError,
    net::SocketAddr,
    sync::{Arc, Mutex},
};
use std::collections::VecDeque;
use std::fs::OpenOptions;
use std::path::PathBuf;

use futures_channel::mpsc::{unbounded, UnboundedSender};
use futures_util::{future, pin_mut, stream::TryStreamExt, StreamExt};
use log::{info, LevelFilter};

use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::tungstenite::protocol::Message;
use crate::steam_util::SteamUtil;
use crate::wine_cask::{AppState, Request, RequestType, Task, TaskType, WineCask};

type Tx = UnboundedSender<Message>;
type PeerMap = Arc<Mutex<HashMap<SocketAddr, Tx>>>;
type AsyncAppState = Arc<Mutex<AppState>>;
type ArcWineCask = Arc<WineCask>;

async fn handle_connection(wine_cask: Arc<WineCask>, peer_map: PeerMap, raw_stream: TcpStream, addr: SocketAddr) {
    println!("Incoming TCP connection from: {}", addr);

    let ws_stream = tokio_tungstenite::accept_async(raw_stream)
        .await
        .expect("Error during the websocket handshake occurred");
    println!("WebSocket connection established: {}", addr);

    // Insert the write part of this peer to the peer map.
    let (tx, rx) = unbounded();
    peer_map.lock().unwrap().insert(addr, tx);

    let (outgoing, incoming) = ws_stream.split();

    let broadcast_incoming = incoming.try_for_each(|msg| {
        println!("Received a message from {}: {}", addr, msg.to_text().unwrap());

        if let Ok(msg) = &msg.to_text() {
            if !msg.is_empty() {
                let request: Request = serde_json::from_str(&msg).unwrap();
                if request.r#type == RequestType::RequestState {
                    wine_cask.update_used_by_games();
                    wine_cask.broadcast_app_state(&peer_map);
                } else if request.r#type == RequestType::Install {
                    wine_cask.add_to_queue(request.install.unwrap());
                } else if request.r#type == RequestType::Uninstall {
                    let uninstall = request.uninstall.unwrap().uninstall;
                    wine_cask.add_to_task_queue(Task{ r#type: TaskType::UninstallCompatibilityTool, uninstall: Some(uninstall) })
                } else if request.r#type == RequestType::Reboot {
                    wine_cask.add_to_task_queue(Task{ r#type: TaskType::Reboot, uninstall: None })
                }
            }
        }

        future::ok(())
    });

    let receive_from_others = rx.map(Ok).forward(outgoing);

    pin_mut!(broadcast_incoming, receive_from_others);
    future::select(broadcast_incoming, receive_from_others).await;

    println!("{} disconnected", &addr);
    peer_map.lock().unwrap().remove(&addr);
}

#[tokio::main]
async fn main() -> Result<(), IoError> {
    // Configure the logger
    let path = env::var("DECKY_PLUGIN_LOG").unwrap_or("/tmp/decky-wine-cellar.log".parse().unwrap()); // Fixme: Probably separate logs

    let target = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .expect("Can't open file");

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

    {
        let addr = env::args().nth(1).unwrap_or_else(|| "127.0.0.1:8887".to_string());

        let state = PeerMap::new(Mutex::new(HashMap::new()));

        let steam_util = SteamUtil::new(PathBuf::from(std::env::var("DECKY_USER_HOME").unwrap()).join(".steam"));

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
        {
            let mut appstate = wine_cask.app_state.lock().unwrap();
            appstate.installed_compatibility_tools = wine_cask.list_compatibility_tools().unwrap();
            appstate.available_flavors = wine_cask.get_flavors(appstate.installed_compatibility_tools.clone(), false).await;
        }

        let wine_cask_arc = ArcWineCask::new(wine_cask);

        tokio::spawn(wine_cask::process_queue(wine_cask_arc.clone(), state.clone()));
        //tokio::spawn(wine_cask::process_tasks(wine_cask_arc.clone(), state.clone()));

        //let async_wine_cask = AsyncWineCask::new(wine_cask);

        // Create the event loop and TCP listener we'll accept connections on.
        let try_socket = TcpListener::bind(&addr).await;
        let listener = try_socket.expect("Failed to bind");
        println!("Listening on: {}", addr);

        // Let's spawn the handling of each connection in a separate task.
        while let Ok((stream, addr)) = listener.accept().await {
            tokio::spawn(handle_connection(wine_cask_arc.clone(), state.clone(), stream, addr));
        }

    }


    info!("Exiting...");
    Ok(())
}
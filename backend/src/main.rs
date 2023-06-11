mod steam_util;
mod wine_cask;

use std::{env, io::Error, io::Write as IoWrite};
use std::fs::OpenOptions;
use bytes::BytesMut;
use env_logger::Env;

use futures_util::{StreamExt};
use log::{error, info, LevelFilter};
use ratchet_rs::{Message, NoExtProvider, PayloadType, ProtocolRegistry, UpgradedServer, WebSocketConfig};
use tokio::net::TcpListener;
use tokio_stream::wrappers::TcpListenerStream;
use crate::wine_cask::{AppState, Request, RequestType, WineCask};

#[tokio::main]
async fn main() -> Result<(), Error> {
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

    websocket_server().await.expect("TODO: panic message");
    info!("Exiting...");
    Ok(())
}

async fn websocket_server() -> Result<(), ratchet_rs::Error> {
    info!("Starting websocket server...");
    let listener = TcpListener::bind("127.0.0.1:8887").await; //Todo: allow port from default settings
    let wine_cask: WineCask = WineCask::new();
    let installed_compatibility_tools = wine_cask.list_compatibility_tools().unwrap();
    let mut app_state: AppState = AppState {
        installed_compatibility_tools,
        in_progress: None,
    };
    match listener {
        Ok(listener) => {
            let mut incoming = TcpListenerStream::new(listener);
            while let Some(socket) = incoming.next().await {
                let socket = socket?;

                // An upgrader contains information about what the peer has requested.
                let upgrader = ratchet_rs::accept_with(
                    socket,
                    WebSocketConfig::default(),
                    NoExtProvider,
                    ProtocolRegistry::default(),
                ).await?;

                let UpgradedServer {
                    request: _,
                    mut websocket,
                    subprotocol: _,
                } = upgrader.upgrade().await?;


                let mut buf = BytesMut::new();
                loop {
                    match websocket.read(&mut buf).await.unwrap() {
                        Message::Text => {
                            let bytes: &[u8] = &buf[..];
                            let msg = String::from_utf8_lossy(bytes).to_string();
                            info!("Websocket message received: {}", msg);
                            let request: Request = serde_json::from_str(&msg).unwrap();
                            if request.r#type == RequestType::RequestState {
                                wine_cask.update_used_by_games(&mut app_state);
                                wine_cask::websocket_update_state(app_state.clone(), &mut websocket).await;
                            } else if request.r#type == RequestType::Install {
                                wine_cask.install_compatibility_tool(request.install.unwrap(), &mut app_state, &mut websocket).await;
                            } else if request.r#type == RequestType::Uninstall {
                                wine_cask.uninstall_compatibility_tool(request.uninstall.unwrap(), &mut app_state, &mut websocket).await;
                            } else if request.r#type == RequestType::Reboot {
                                app_state.installed_compatibility_tools = wine_cask.list_compatibility_tools().unwrap();
                                wine_cask::websocket_update_state(app_state.clone(), &mut websocket).await;
                            }
                            //websocket.write(&mut buf, PayloadType::Text).await.unwrap();
                            buf.clear();
                        }
                        Message::Binary => {
                            websocket.write(&mut buf, PayloadType::Binary).await.unwrap();
                            buf.clear();
                        }
                        Message::Ping(_) | Message::Pong(_) => {
                            // Ping messages are transparently handled by Ratchet
                        }
                        Message::Close(reason) => {
                            if let Some(reason) = reason {
                                if let Some(reason_description) = &reason.description {
                                    info!("Closed websocket connection! Reason: {}", reason_description);
                                } else {
                                    info!("Closed websocket connection!");
                                }
                            } else {
                                info!("Closed websocket connection!");
                            }
                            break;
                        }
                    }
                }
            }
        }
        Err(error) => {
            error!("{}", error);
        }
    }

    Ok(())
}
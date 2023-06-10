mod steam;
mod wine_cellar;

use std::{env, io::Error, io::Write as IoWrite};
use std::fs::OpenOptions;
use std::path::Path;
use bytes::BytesMut;
use env_logger::Env;

use futures_util::{StreamExt};
use log::{error, info, LevelFilter};
use ratchet_rs::{Message, NoExtProvider, PayloadType, ProtocolRegistry, UpgradedServer, WebSocketConfig};
use tokio::net::TcpListener;
use tokio_stream::wrappers::TcpListenerStream;
use crate::steam::SteamCompatibilityTool;
use crate::wine_cellar::{Response, ResponseType};

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
    match listener {
        Ok(listener) => {
            let mut incoming = TcpListenerStream::new(listener);

            let path = env::var("DECKY_USER_HOME").unwrap_or("/home/deck".parse().unwrap()); // we don't know and fall back to /home/deck
            let path = Path::new(&path).join(".steam").join("root").join("compatibilitytools.d");
            let mut internal_installed: Vec<SteamCompatibilityTool> = steam::get_installed_compatibility_tools(&path);//Vec::new(); // must be one time query on launch and updated.

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
                            let response: Response = serde_json::from_str(&msg).unwrap();
                            if response.r#type == ResponseType::Install {
                                wine_cellar::install_compatibility_tool(&path, &response, &mut internal_installed, &mut websocket).await;
                            } else if response.r#type == ResponseType::RequestState {
                                wine_cellar::websocket_update_state(internal_installed.clone(), None, &mut websocket).await;
                            } else if response.r#type == ResponseType::Uninstall {
                                wine_cellar::uninstall_compatibility_tool(&path, &response.name.unwrap(), &mut internal_installed, &mut websocket).await;
                            } else if response.r#type == ResponseType::Reboot {
                                internal_installed = steam::get_installed_compatibility_tools(&path);
                                wine_cellar::websocket_update_state(internal_installed.clone(), None, &mut websocket).await;
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
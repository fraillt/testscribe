use std::collections::BTreeMap;
use std::env::var;
use std::mem::take;
use std::process::ExitCode;

use futures::future::{FusedFuture, ready};
use futures::stream::{SplitSink, SplitStream};
use futures::{FutureExt, SinkExt, StreamExt, select};
use serde::{Deserialize, Serialize};
use tokio::net::TcpStream;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async};
use tungstenite::{ClientRequestBuilder, Error, Message};

use crate::driver::{CommandSender, Frontend, StatusReceiver};
use crate::runtime::messages::CommandMsg;
use testscribe_core::test_case::FqFnName;
use testscribe_core::tests_tree::TestsTree;

pub const REMOTE_FRONTEND_URL: &str = "TESTSCRIBE_REMOTE_FRONTEND_URL";
pub const REMOTE_FRONTEND_SESSION_NAME: &str = "TESTSCRIBE_REMOTE_FRONTENT_SESSION_NAME";
pub const DEFAULT_REMOTE_FRONTEND_URL: &str = "ws://localhost:5173/ws";
pub const DEFAULT_REMOTE_FRONTEND_SESSION_NAME: &str = "default";

pub struct RemoteFrontend {
    send_stream: SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>,
    receive_stream: SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>,
}

impl RemoteFrontend {
    pub async fn try_connect(url: String, session_name: String) -> Option<Self> {
        let req = ClientRequestBuilder::new(url.parse().unwrap())
            .with_sub_protocol("publisher")
            .with_header("session-name", session_name);

        let resp = connect_async(req).await;
        match resp {
            Ok((ws, _resp)) => {
                let (send_stream, receive_stream) = ws.split();
                Some(Self {
                    send_stream,
                    receive_stream,
                })
            }
            Err(Error::Io(_err)) => None,
            Err(err) => {
                println!("Error connecting to remote frontend: {err}");
                None
            }
        }
    }

    pub async fn try_from_env() -> Option<Self> {
        let url = var(REMOTE_FRONTEND_URL)
            .ok()
            .unwrap_or_else(|| DEFAULT_REMOTE_FRONTEND_URL.to_string());
        let session_name = var(REMOTE_FRONTEND_SESSION_NAME)
            .ok()
            .unwrap_or_else(|| DEFAULT_REMOTE_FRONTEND_SESSION_NAME.to_string());
        Self::try_connect(url, session_name).await
    }
}

#[derive(Serialize)]
#[serde(tag = "kind")]
pub enum StartMsg<'a> {
    Init {
        root_name: FqFnName<'static>,
        tree: &'a TestsTree,
    },
}

#[derive(Deserialize)]
pub struct CloseMsg {
    successful: bool,
}

impl Frontend for RemoteFrontend {
    async fn start(
        mut self,
        dags: BTreeMap<FqFnName<'static>, TestsTree>,
        command_sender: CommandSender,
        mut status_receiver: StatusReceiver,
    ) -> ExitCode {
        let mut receive_cmd = self.receive_stream.next().fuse();
        let mut receive_status = status_receiver.next();
        let mut list_to_deliver = Vec::new();
        let msg = serde_json::to_string(
            &dags
                .values()
                .map(|tree| StartMsg::Init {
                    root_name: tree.node.name,
                    tree: tree,
                })
                .collect::<Vec<_>>(),
        )
        .unwrap();
        eprintln!("****** sending dag: {msg}");
        let mut deliver_fut = self
            .send_stream
            .send(tungstenite::Message::Text(msg.into()))
            .boxed()
            .fuse();

        let exit_code = loop {
            select! {
                cmd = receive_cmd => {
                    eprintln!("********* receive cmd:");
                    if let Some(cmd) = cmd {
                        match cmd {
                            Ok(Message::Text(utf8_bytes)) => {
                                eprintln!("********* receive msg: {}",utf8_bytes.as_str());
                                let cmd: CommandMsg = serde_json::from_str(utf8_bytes.as_str()).unwrap();
                                command_sender.unbounded_send(cmd).unwrap();
                            },
                            Ok(Message::Close(close_frame)) => {
                                let exit_code = if let Some(frame) = close_frame {
                                    match serde_json::from_str::<CloseMsg>(frame.reason.as_str()) {
                                        Ok(msg) => if msg.successful {
                                            ExitCode::SUCCESS
                                        } else {
                                            ExitCode::FAILURE
                                        },
                                        Err(err) => {
                                            eprintln!("Invalid close message format: {}", err);
                                            ExitCode::FAILURE
                                        }
                                    }
                                } else {
                                    eprintln!("Missing close status");
                                    ExitCode::FAILURE
                                };
                                eprintln!("************ close!!!");
                                break exit_code;
                            }
                            Ok(_) => {
                                eprintln!("...")
                            }
                            Err(err) => eprintln!("********* receive err: {err:?}")
                        }
                    } else {
                        eprintln!("Lost connection to remote frontend");
                        // connection lost to remote server
                        break ExitCode::FAILURE;
                    }
                    receive_cmd = self.receive_stream.next().fuse();
                },
                status = receive_status => {
                    eprintln!("......... receive status:");
                    if let Some(msg) = status {
                        let msg = serde_json::to_value(&msg).unwrap();
                        list_to_deliver.push(msg);
                        if deliver_fut.is_terminated() {
                            // trigger redelivering
                            eprintln!("********* trigger deliver fut");
                            deliver_fut = ready(Ok(())).boxed().fuse();
                        }
                    } else {
                        eprintln!("************** received no status msg, maybe it's closed?");
                    }
                    receive_status = status_receiver.next();
                }
                s = deliver_fut => {
                    eprintln!("********* send msg response: {s:?}");
                    let send_list = take(&mut list_to_deliver);
                    if !send_list.is_empty() {
                        eprintln!("start sending {} msgs", send_list.len());
                        let msg = serde_json::to_string(&send_list).unwrap();
                        drop(deliver_fut);
                        deliver_fut = self.send_stream.send(tungstenite::Message::Text(msg.into())).boxed().fuse();
                    }
                }
            }
        };
        // even if connection was closed by remote server, we still need to poll websocket in order to finish close flow properly
        if deliver_fut.is_terminated() {
            drop(deliver_fut);
        } else {
            deliver_fut.await.unwrap();
        }
        self.send_stream.close().await.unwrap();
        exit_code
    }
}

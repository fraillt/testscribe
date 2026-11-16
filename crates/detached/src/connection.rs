//! The one connection of a test binary to the frontend. It lives on a thread of its own, so
//! that it serves sync and async trees alike, whatever runtime they run on.

use std::collections::HashMap;
use std::sync::OnceLock;
use std::time::Duration;

use futures::channel::oneshot;
use futures::{SinkExt, StreamExt};
use testscribe_core::test_case::TestCase;
use testscribe_core::tests_tree::create_test_trees;
use testscribe_standalone::Selection;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

use crate::protocol::{FromFrontend, PROTOCOL_VERSION, Select, ToFrontend};
use crate::{BATCH_ENV_NAME, FRONTEND_ENV_NAME, RUN_ENV_NAME};

/// How long a frontend may take to accept the connection.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(3);
/// How long a frontend may take to say what to run, before everything runs.
const SELECT_TIMEOUT: Duration = Duration::from_secs(5);

pub enum Outgoing {
    /// A message for the frontend, already written out.
    Message(String),
    /// A tree is ready and waits for `reply`.
    Ready {
        root: String,
        reply: oneshot::Sender<Selection>,
    },
    /// Tells when everything before it has been handed to the network.
    Flush(oneshot::Sender<()>),
}

pub struct Frontend {
    outgoing: UnboundedSender<Outgoing>,
}

impl Frontend {
    pub fn send(&self, outgoing: Outgoing) {
        // The connection thread only ends with the process.
        let _ = self.outgoing.send(outgoing);
    }
}

/// The frontend this test binary reports to, if one is named and answers. Connects on the
/// first call.
pub fn frontend(test_cases: &'static [TestCase]) -> Option<&'static Frontend> {
    static FRONTEND: OnceLock<Option<Frontend>> = OnceLock::new();
    FRONTEND
        .get_or_init(|| {
            let address = std::env::var(FRONTEND_ENV_NAME).ok()?;
            let url = if address.contains("://") {
                address
            } else {
                format!("ws://{address}/tests")
            };
            match connect(&url, test_cases) {
                Ok(frontend) => Some(frontend),
                Err(reason) => {
                    eprintln!(
                        "testscribe: no frontend at {url} ({reason}); the tests run without it"
                    );
                    None
                }
            }
        })
        .as_ref()
}

fn connect(url: &str, test_cases: &'static [TestCase]) -> Result<Frontend, String> {
    let (outgoing, to_send) = unbounded_channel();
    let (connected, connecting) = std::sync::mpsc::channel();
    let url = url.to_string();
    std::thread::Builder::new()
        .name("testscribe-frontend".to_string())
        .spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("a runtime for the connection to the frontend");
            runtime.block_on(async move {
                match tokio::time::timeout(CONNECT_TIMEOUT, connect_async(url.as_str())).await {
                    Ok(Ok((socket, _response))) => {
                        let _ = connected.send(Ok(()));
                        serve(socket, to_send, hello(test_cases)).await;
                    }
                    Ok(Err(err)) => {
                        let _ = connected.send(Err(err.to_string()));
                    }
                    Err(_) => {
                        let _ = connected.send(Err("it did not answer in time".to_string()));
                    }
                }
            });
        })
        .map_err(|err| err.to_string())?;
    connecting
        .recv()
        .map_err(|_| "the connection could not be started".to_string())?
        .map(|()| Frontend { outgoing })
}

fn hello(test_cases: &'static [TestCase]) -> String {
    let binary = std::env::current_exe()
        .ok()
        .and_then(|path| {
            path.file_name()
                .map(|name| name.to_string_lossy().into_owned())
        })
        .unwrap_or_default();
    ToFrontend::Hello {
        protocol: PROTOCOL_VERSION,
        binary,
        batch: std::env::var(BATCH_ENV_NAME)
            .ok()
            .filter(|batch| !batch.is_empty()),
        run: std::env::var(RUN_ENV_NAME)
            .ok()
            .filter(|run| !run.is_empty()),
        invocation: invocation(),
        manifest_dir: std::env::var("CARGO_MANIFEST_DIR").ok(),
        cwd: std::env::current_dir()
            .ok()
            .map(|dir| dir.to_string_lossy().into_owned()),
        pid: std::process::id(),
        trees: &create_test_trees(test_cases),
    }
    .to_json()
}

#[cfg(unix)]
fn invocation() -> Option<u32> {
    Some(std::os::unix::process::parent_id())
}

#[cfg(not(unix))]
fn invocation() -> Option<u32> {
    None
}

async fn serve<S>(socket: S, mut to_send: UnboundedReceiver<Outgoing>, hello: String)
where
    S: futures::Sink<Message> + futures::Stream<Item = Result<Message, S::Error>> + Unpin,
{
    let (mut sink, mut stream) = socket.split();
    let mut waiting: HashMap<String, oneshot::Sender<Selection>> = HashMap::new();
    let (timed_out, mut timeouts) = unbounded_channel::<String>();

    let mut connected = sink.send(Message::text(hello)).await.is_ok();
    while connected {
        tokio::select! {
            outgoing = to_send.recv() => match outgoing {
                Some(Outgoing::Message(message)) => {
                    connected = sink.send(Message::text(message)).await.is_ok();
                }
                Some(Outgoing::Ready { root, reply }) => {
                    let ready = ToFrontend::TreeReady { root: &root }.to_json();
                    connected = sink.send(Message::text(ready)).await.is_ok();
                    let timed_out = timed_out.clone();
                    let waiting_root = root.clone();
                    tokio::spawn(async move {
                        tokio::time::sleep(SELECT_TIMEOUT).await;
                        let _ = timed_out.send(waiting_root);
                    });
                    waiting.insert(root, reply);
                }
                Some(Outgoing::Flush(done)) => {
                    connected = sink.flush().await.is_ok();
                    let _ = done.send(());
                }
                None => return,
            },
            incoming = stream.next() => match incoming {
                Some(Ok(Message::Text(text))) => {
                    if let Ok(FromFrontend::Run { root, select }) = serde_json::from_str(&text)
                        && let Some(reply) = waiting.remove(&root)
                    {
                        let _ = reply.send(match select {
                            Select::All => Selection::All,
                            Select::Paths { paths } => Selection::Paths(paths),
                            Select::None => Selection::None,
                        });
                    }
                }
                Some(Ok(_)) => {}
                Some(Err(_)) | None => connected = false,
            },
            Some(root) = timeouts.recv() => {
                if let Some(reply) = waiting.remove(&root) {
                    let _ = reply.send(Selection::All);
                }
            }
        }
    }

    // The frontend is gone. The tests go on without it: whoever still asks is told to run
    // everything, and nothing is left to wait for.
    for (_root, reply) in waiting.drain() {
        let _ = reply.send(Selection::All);
    }
    while let Some(outgoing) = to_send.recv().await {
        match outgoing {
            Outgoing::Message(_) => {}
            Outgoing::Ready { reply, .. } => {
                let _ = reply.send(Selection::All);
            }
            Outgoing::Flush(done) => {
                let _ = done.send(());
            }
        }
    }
}

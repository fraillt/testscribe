//! The testscribe frontend server.
//!
//! Test binaries connect to `/tests` and report their trees and every test they run (the
//! protocol is described in the `testscribe-detached` crate). The UI connects to `/ui` and
//! is sent everything known so far, then everything as it happens. The same port serves the
//! built UI, so a browser is all it takes to look at a run.
//!
//! The server keeps what it is told as it was told: a session per test binary, with its
//! messages in order. It reads a message only as far as it has to, to answer it.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use axum::Router;
use axum::extract::State;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::response::Response;
use axum::routing::get;
use futures::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tokio::net::TcpListener;
use tokio::sync::broadcast;
use tower_http::services::{ServeDir, ServeFile};

/// The port test binaries and browsers find the frontend at, unless told otherwise.
pub const DEFAULT_PORT: u16 = 7357;

/// One test binary that connected, and what it said.
struct Session {
    id: u64,
    /// Its first message, with its test trees.
    hello: Value,
    /// Every message after that, in order.
    events: Vec<Value>,
    /// Whether the test binary is still connected.
    open: bool,
}

impl Session {
    fn to_json(&self) -> Value {
        json!({
            "id": self.id,
            "hello": self.hello,
            "events": self.events,
            "open": self.open,
        })
    }
}

struct Hub {
    sessions: Vec<Session>,
    /// What every connected UI is sent, as it happens.
    to_ui: broadcast::Sender<String>,
}

impl Hub {
    fn tell_ui(&self, message: Value) {
        // Nobody listening is fine: a UI that connects later is sent everything so far.
        let _ = self.to_ui.send(message.to_string());
    }

    fn session(&mut self, id: u64) -> &mut Session {
        self.sessions
            .iter_mut()
            .find(|session| session.id == id)
            .expect("a session is never removed while its test binary is connected")
    }
}

#[derive(Clone)]
pub struct Frontend {
    hub: Arc<Mutex<Hub>>,
}

impl Default for Frontend {
    fn default() -> Self {
        Self::new()
    }
}

impl Frontend {
    pub fn new() -> Self {
        let (to_ui, _) = broadcast::channel(4096);
        Self {
            hub: Arc::new(Mutex::new(Hub {
                sessions: Vec::new(),
                to_ui,
            })),
        }
    }

    /// The routes of the frontend. With `ui_dir`, the built UI is served from there.
    pub fn router(&self, ui_dir: Option<PathBuf>) -> Router {
        let router = Router::new()
            .route("/tests", get(tests_connected))
            .route("/ui", get(ui_connected))
            .with_state(self.clone());
        match ui_dir {
            Some(dir) => {
                let index = ServeFile::new(dir.join("index.html"));
                router.fallback_service(ServeDir::new(dir).fallback(index))
            }
            None => router,
        }
    }

    /// Serves the frontend until the process ends.
    pub async fn serve(
        self,
        listener: TcpListener,
        ui_dir: Option<PathBuf>,
    ) -> std::io::Result<()> {
        axum::serve(listener, self.router(ui_dir)).await
    }
}

async fn tests_connected(State(frontend): State<Frontend>, upgrade: WebSocketUpgrade) -> Response {
    upgrade.on_upgrade(move |socket| tests_session(frontend, socket))
}

async fn ui_connected(State(frontend): State<Frontend>, upgrade: WebSocketUpgrade) -> Response {
    upgrade.on_upgrade(move |socket| ui_session(frontend, socket))
}

async fn next_json(socket: &mut WebSocket) -> Option<Value> {
    loop {
        match socket.recv().await? {
            Ok(Message::Text(text)) => {
                if let Ok(message) = serde_json::from_str(text.as_str()) {
                    return Some(message);
                }
            }
            Ok(Message::Close(_)) | Err(_) => return None,
            Ok(_) => {}
        }
    }
}

async fn tests_session(frontend: Frontend, mut socket: WebSocket) {
    let Some(hello) = next_json(&mut socket).await else {
        return;
    };
    let id = {
        let mut hub = frontend.hub.lock().unwrap();
        let id = hub.sessions.last().map_or(1, |session| session.id + 1);
        let session = Session {
            id,
            hello,
            events: Vec::new(),
            open: true,
        };
        hub.tell_ui(json!({ "kind": "session_opened", "session": session.to_json() }));
        hub.sessions.push(session);
        id
    };

    while let Some(event) = next_json(&mut socket).await {
        // A tree that is ready waits to be told what to run. For now that is everything.
        let answer = (event["kind"] == "tree_ready")
            .then(|| json!({ "kind": "run", "root": event["root"], "select": { "kind": "all" } }));
        {
            let mut hub = frontend.hub.lock().unwrap();
            hub.tell_ui(json!({ "kind": "event", "session": id, "event": event }));
            hub.session(id).events.push(event);
        }
        if let Some(answer) = answer
            && socket
                .send(Message::Text(answer.to_string().into()))
                .await
                .is_err()
        {
            break;
        }
    }

    let mut hub = frontend.hub.lock().unwrap();
    hub.session(id).open = false;
    hub.tell_ui(json!({ "kind": "session_closed", "session": id }));
}

async fn ui_session(frontend: Frontend, socket: WebSocket) {
    // Everything so far and the subscription to what follows are taken in one step, so that
    // nothing falls between them.
    let (snapshot, mut updates) = {
        let hub = frontend.hub.lock().unwrap();
        let sessions: Vec<Value> = hub.sessions.iter().map(Session::to_json).collect();
        (
            json!({ "kind": "snapshot", "sessions": sessions }),
            hub.to_ui.subscribe(),
        )
    };
    let (mut to_ui, mut from_ui) = socket.split();
    if to_ui
        .send(Message::Text(snapshot.to_string().into()))
        .await
        .is_err()
    {
        return;
    }
    loop {
        tokio::select! {
            update = updates.recv() => match update {
                Ok(update) => {
                    if to_ui.send(Message::Text(update.into())).await.is_err() {
                        return;
                    }
                }
                // A UI too slow to keep up has lost messages and would show a wrong run.
                // Closing makes it connect again and start from a snapshot.
                Err(broadcast::error::RecvError::Lagged(_)) => return,
                Err(broadcast::error::RecvError::Closed) => return,
            },
            message = from_ui.next() => match message {
                Some(Ok(Message::Text(text))) => {
                    if serde_json::from_str::<Value>(text.as_str())
                        .is_ok_and(|command| command["kind"] == "clear")
                    {
                        let mut hub = frontend.hub.lock().unwrap();
                        hub.sessions.retain(|session| session.open);
                        let sessions: Vec<Value> = hub.sessions.iter().map(Session::to_json).collect();
                        hub.tell_ui(json!({ "kind": "snapshot", "sessions": sessions }));
                    }
                }
                Some(Ok(_)) => {}
                Some(Err(_)) | None => return,
            },
        }
    }
}

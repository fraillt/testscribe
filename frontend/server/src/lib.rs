//! The testscribe frontend server.
//!
//! Test binaries connect to `/tests` and report their trees and every test they run (the
//! protocol is described in the `testscribe-detached` crate). The UI connects to `/ui` and
//! is sent everything known so far, then everything as it happens. The same port serves the
//! built UI, so a browser is all it takes to look at a run.
//!
//! The server keeps what it is told as it was told: a session per test binary, with its
//! messages in order.
//!
//! Next to that it keeps batches. A run belongs to the batch it names, or to
//! [`DEFAULT_BATCH`], and a batch is what every test in it said the last time it ran (see
//! [`outcome`] and [`store`]). Two batches can be compared, and a batch says what its tests
//! run the next time they ask: everything, the tests picked in the UI, or the new and the
//! failed ones.

pub mod outcome;
pub mod store;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use axum::Router;
use axum::extract::State;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::response::Response;
use axum::routing::get;
use futures::{SinkExt, StreamExt};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::net::TcpListener;
use tokio::sync::broadcast;
use tower_http::services::{ServeDir, ServeFile};

use crate::outcome::{Status, TestPath, TreeOutcome};
pub use crate::store::DEFAULT_BATCH;
use crate::store::{RunMode, Store, TestId};

/// The port test binaries and browsers find the frontend at, unless told otherwise.
pub const DEFAULT_PORT: u16 = 7357;

/// A tree of a session that is running, or ran without asking what to run.
#[derive(Default)]
struct TreeRun {
    /// Whether the tree was told to run all of it.
    whole_tree: bool,
    outcome: TreeOutcome,
}

/// One test binary that connected, and what it said.
struct Session {
    id: u64,
    /// The batch its results belong to.
    batch: String,
    /// The name of the test binary, without the hash cargo adds.
    binary: String,
    /// Its first message, with its test trees.
    hello: Value,
    /// Every message after that, in order.
    events: Vec<Value>,
    /// Whether the test binary is still connected.
    open: bool,
    /// Every test of the binary, with its place in its tree.
    place: HashMap<TestPath, usize>,
    /// The trees that have not finished, by root.
    running: HashMap<String, TreeRun>,
}

impl Session {
    fn new(id: u64, hello: Value) -> Self {
        let batch = match hello["batch"].as_str() {
            Some(batch) if !batch.is_empty() => batch.to_string(),
            _ => DEFAULT_BATCH.to_string(),
        };
        let file_name = hello["binary"].as_str().unwrap_or_default();
        let binary = match file_name.rsplit_once('-') {
            Some((name, hash))
                if hash.len() == 16 && hash.bytes().all(|byte| byte.is_ascii_hexdigit()) =>
            {
                name
            }
            _ => file_name,
        };
        let mut place = HashMap::new();
        for tree in hello["trees"].as_array().into_iter().flatten() {
            let mut nr = 0;
            number_tests(tree, &mut Vec::new(), &mut nr, &mut place);
        }
        Self {
            id,
            batch,
            binary: binary.to_string(),
            hello,
            events: Vec::new(),
            open: true,
            place,
            running: HashMap::new(),
        }
    }

    fn to_json(&self) -> Value {
        json!({
            "id": self.id,
            "batch": self.batch,
            "hello": self.hello,
            "events": self.events,
            "open": self.open,
        })
    }
}

/// Gives every test of `tree` its number, in the order the tests are written.
fn number_tests(
    tree: &Value,
    above: &mut TestPath,
    nr: &mut usize,
    place: &mut HashMap<TestPath, usize>,
) {
    let name = &tree["node"]["name"];
    above.push(format!(
        "{}::{}",
        name["path"].as_str().unwrap_or_default(),
        name["name"].as_str().unwrap_or_default()
    ));
    place.insert(above.clone(), *nr);
    *nr += 1;
    for child in tree["childs"].as_array().into_iter().flatten() {
        number_tests(child, above, nr, place);
    }
    above.pop();
}

/// What the UI may ask for.
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum FromUi {
    /// Forget the sessions that are over.
    Clear,
    GetBatch {
        name: String,
    },
    DeleteBatch {
        name: String,
    },
    /// What the batch runs the next time. Without `selected`, the tests picked earlier stay.
    SetRun {
        batch: String,
        mode: RunMode,
        selected: Option<Vec<TestId>>,
    },
    Diff {
        from: String,
        to: String,
    },
}

struct Hub {
    sessions: Vec<Session>,
    store: Store,
    /// What every connected UI is sent, as it happens.
    to_ui: broadcast::Sender<String>,
}

/// A database that fails does not stop the tests from being shown.
fn or_complain<T: Default>(result: rusqlite::Result<T>) -> T {
    result.unwrap_or_else(|err| {
        eprintln!("testscribe frontend: the batches could not be read or written: {err}");
        T::default()
    })
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

    fn batches(&self) -> Value {
        json!(or_complain(self.store.batches()))
    }

    fn tell_batches(&self) {
        self.tell_ui(json!({ "kind": "batches", "batches": self.batches() }));
    }

    fn snapshot(&self) -> Value {
        let sessions: Vec<Value> = self.sessions.iter().map(Session::to_json).collect();
        json!({ "kind": "snapshot", "sessions": sessions, "batches": self.batches() })
    }

    fn opened(&mut self, session: Session) {
        let existing: HashSet<TestPath> = session.place.keys().cloned().collect();
        or_complain(
            self.store
                .binary_reports(&session.batch, &session.binary, &existing),
        );
        self.tell_ui(json!({ "kind": "session_opened", "session": session.to_json() }));
        self.tell_batches();
        self.sessions.push(session);
    }

    /// Takes a message of a test binary, and returns the answer to it, if it needs one.
    fn event(&mut self, id: u64, event: Value) -> Option<Value> {
        self.tell_ui(json!({ "kind": "event", "session": id, "event": event }));
        let root = event["root"].as_str().unwrap_or_default().to_string();
        let mut answer = None;
        match event["kind"].as_str() {
            // A tree that is ready waits to be told what to run.
            Some("tree_ready") => {
                let select = self.select(id, &root);
                let whole_tree = select["kind"] == "all";
                let tree = self.session(id).running.entry(root).or_default();
                tree.whole_tree = whole_tree;
                answer = Some(json!({ "kind": "run", "root": event["root"], "select": select }));
            }
            Some("test") => {
                let tree = self.session(id).running.entry(root).or_default();
                tree.outcome.apply(&event);
            }
            Some("tree_finished") => {
                self.save_tree(id, &root);
                self.tell_batches();
            }
            _ => {}
        }
        self.session(id).events.push(event);
        answer
    }

    /// What the tree `root` of a session runs, as its batch has it.
    fn select(&mut self, id: u64, root: &str) -> Value {
        let session = self
            .sessions
            .iter()
            .find(|session| session.id == id)
            .expect("the session of a tree that is ready is there");
        let (mode, selected) = or_complain(self.store.run(&session.batch).map(Some))
            .unwrap_or((RunMode::All, Vec::new()));
        let paths: Vec<TestPath> = match mode {
            RunMode::All => return json!({ "kind": "all" }),
            RunMode::Selected => selected
                .into_iter()
                .filter(|test| test.binary == session.binary)
                .map(|test| test.path)
                .filter(|path| path.first().is_some_and(|name| name == root))
                .collect(),
            RunMode::NewAndFailed => {
                let known = or_complain(self.store.statuses(&session.batch, &session.binary, root));
                if known.is_empty() {
                    // All of the tree is new.
                    return json!({ "kind": "all" });
                }
                let mut paths: Vec<_> = session
                    .place
                    .iter()
                    .filter(|(path, _)| path.first().is_some_and(|name| name == root))
                    .filter(|(path, _)| known.get(*path) != Some(&Status::Passed))
                    .collect();
                paths.sort_by_key(|(_, nr)| **nr);
                paths.into_iter().map(|(path, _)| path.clone()).collect()
            }
        };
        if paths.is_empty() {
            json!({ "kind": "none" })
        } else {
            json!({ "kind": "paths", "paths": paths })
        }
    }

    /// The tree is over: what its tests said goes to the batch.
    fn save_tree(&mut self, id: u64, root: &str) {
        let Some(session) = self.sessions.iter_mut().find(|session| session.id == id) else {
            return;
        };
        let Some(tree) = session.running.remove(root) else {
            return;
        };
        or_complain(self.store.save_tree(
            &session.batch,
            &session.binary,
            root,
            tree.whole_tree,
            tree.outcome.tests(),
            &session.place,
        ));
    }

    fn closed(&mut self, id: u64) {
        // A test binary that went away in the middle of a tree: what ran, ran.
        let unfinished: Vec<String> = self.session(id).running.keys().cloned().collect();
        for root in &unfinished {
            self.session(id)
                .running
                .get_mut(root)
                .expect("the tree was just listed")
                .whole_tree = false;
            self.save_tree(id, root);
        }
        self.session(id).open = false;
        self.tell_ui(json!({ "kind": "session_closed", "session": id }));
        if !unfinished.is_empty() {
            self.tell_batches();
        }
    }

    /// Does what a UI asked for, and returns what only that UI is to be told.
    fn asked(&mut self, command: FromUi) -> Option<Value> {
        match command {
            FromUi::Clear => {
                self.sessions.retain(|session| session.open);
                self.tell_ui(self.snapshot());
                None
            }
            FromUi::GetBatch { name } => {
                let batch = or_complain(self.store.batch(&name));
                Some(json!({ "kind": "batch", "name": name, "batch": batch }))
            }
            FromUi::DeleteBatch { name } => {
                or_complain(self.store.delete(&name));
                self.tell_batches();
                None
            }
            FromUi::SetRun {
                batch,
                mode,
                selected,
            } => {
                or_complain(self.store.set_run(&batch, mode, selected.as_deref()));
                self.tell_batches();
                None
            }
            FromUi::Diff { from, to } => {
                let changes = or_complain(self.store.diff(&from, &to));
                Some(json!({ "kind": "diff", "from": from, "to": to, "changes": changes }))
            }
        }
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
    /// A frontend that stores nothing: all its batches are gone with it.
    pub fn new() -> Self {
        Self::with_store(Store::open(None).expect("a database in memory can be opened"))
    }

    /// A frontend that stores its named batches in the SQLite database at `path`, which is
    /// created if it is not there.
    pub fn with_database(path: &Path) -> rusqlite::Result<Self> {
        Store::open(Some(path)).map(Self::with_store)
    }

    fn with_store(store: Store) -> Self {
        let (to_ui, _) = broadcast::channel(4096);
        Self {
            hub: Arc::new(Mutex::new(Hub {
                sessions: Vec::new(),
                store,
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
        hub.opened(Session::new(id, hello));
        id
    };

    while let Some(event) = next_json(&mut socket).await {
        let answer = frontend.hub.lock().unwrap().event(id, event);
        if let Some(answer) = answer
            && socket
                .send(Message::Text(answer.to_string().into()))
                .await
                .is_err()
        {
            break;
        }
    }

    frontend.hub.lock().unwrap().closed(id);
}

async fn ui_session(frontend: Frontend, socket: WebSocket) {
    // Everything so far and the subscription to what follows are taken in one step, so that
    // nothing falls between them.
    let (snapshot, mut updates) = {
        let hub = frontend.hub.lock().unwrap();
        (hub.snapshot(), hub.to_ui.subscribe())
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
                    let Ok(command) = serde_json::from_str::<FromUi>(text.as_str()) else {
                        continue;
                    };
                    let reply = frontend.hub.lock().unwrap().asked(command);
                    if let Some(reply) = reply
                        && to_ui.send(Message::Text(reply.to_string().into())).await.is_err()
                    {
                        return;
                    }
                }
                Some(Ok(_)) => {}
                Some(Err(_)) | None => return,
            },
        }
    }
}

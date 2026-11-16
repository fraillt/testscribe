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
//!
//! Where it is allowed to ([`Frontend::allow_commands`]), the server also runs the tests of
//! a batch itself, when the UI asks for it: see [`command`].

pub mod command;

pub mod outcome;
pub mod store;

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::Router;
use axum::extract::State;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use futures::{SinkExt, StreamExt};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::net::TcpListener;
use tokio::sync::{Notify, broadcast};
use tower_http::services::{ServeDir, ServeFile};

use crate::outcome::{Status, TestPath, TreeOutcome};
pub use crate::store::DEFAULT_BATCH;
use crate::store::{Announced, RunMode, Sources, Store, TestId};

/// The port test binaries and browsers find the frontend at, unless told otherwise.
pub const DEFAULT_PORT: u16 = 7357;

/// How long the UIs may be behind on what the batches know, while tests report.
const BATCHES_TOLD_EVERY: Duration = Duration::from_millis(300);

/// How many lines of the output of a command are kept.
const OUTPUT_KEPT: usize = 2000;

/// One test binary that connected, and what it said.
struct Session {
    id: u64,
    /// The batch its results belong to.
    batch: String,
    /// The name of the test binary, without the hash cargo adds.
    binary: String,
    /// What to run, as the run itself says it: it comes before what the batch is set to.
    run: Option<RunMode>,
    /// Where the workspace of the tests is, if it is on this machine.
    workspace_dir: Option<String>,
    /// Its first message, with its test trees.
    hello: Value,
    /// Every message after that, in order.
    events: Vec<Value>,
    /// Whether the test binary is still connected.
    open: bool,
    /// Every test of the binary, in the order the tests are written.
    announced: Vec<Announced>,
    /// The trees that have not finished, by root.
    running: HashMap<String, TreeOutcome>,
}

impl Session {
    fn new(id: u64, hello: Value, workspace_dir: Option<String>) -> Self {
        let batch = match hello["batch"].as_str() {
            Some(batch) if !batch.is_empty() => batch.to_string(),
            _ => DEFAULT_BATCH.to_string(),
        };
        let run = match hello["run"].as_str() {
            Some("all") => Some(RunMode::All),
            Some("new-and-failed" | "new_and_failed") => Some(RunMode::NewAndFailed),
            _ => None,
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
        let mut announced = Vec::new();
        for tree in hello["trees"].as_array().into_iter().flatten() {
            announce(tree, &mut Vec::new(), &mut announced);
        }
        Self {
            id,
            batch,
            binary: binary.to_string(),
            run,
            workspace_dir,
            hello,
            events: Vec::new(),
            open: true,
            announced,
            running: HashMap::new(),
        }
    }

    fn to_json(&self) -> Value {
        json!({
            "id": self.id,
            "batch": self.batch,
            "workspace_dir": self.workspace_dir,
            "hello": self.hello,
            "events": self.events,
            "open": self.open,
        })
    }
}

/// Lists every test of `tree`, in the order the tests are written.
fn announce(tree: &Value, above: &mut TestPath, announced: &mut Vec<Announced>) {
    let node = &tree["node"];
    above.push(format!(
        "{}::{}",
        node["name"]["path"].as_str().unwrap_or_default(),
        node["name"]["name"].as_str().unwrap_or_default()
    ));
    announced.push(Announced {
        path: above.clone(),
        ord: announced.len(),
        file: node["filename"].as_str().unwrap_or_default().to_string(),
        line: node["line_nr"].as_u64().unwrap_or(0) as u32,
    });
    for child in tree["childs"].as_array().into_iter().flatten() {
        announce(child, above, announced);
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
    /// Forget what the batch knows of a test binary.
    DeleteBinary {
        batch: String,
        binary: String,
    },
    /// What the batch runs the next time. Without `selected`, the tests picked earlier stay.
    SetRun {
        batch: String,
        mode: RunMode,
        selected: Option<Vec<TestId>>,
    },
    /// What "run again" executes for the batch, and where.
    SetCommand {
        batch: String,
        command: String,
        dir: String,
    },
    /// Run the tests of the batch, with its command.
    RunBatch {
        batch: String,
    },
    StopBatch {
        batch: String,
    },
    Diff {
        from: String,
        to: String,
    },
}

/// The command of a batch, running or over.
#[derive(Default)]
struct CommandRun {
    running: bool,
    /// How the command ended, if it did and said so.
    code: Option<i32>,
    /// The last lines it printed.
    output: VecDeque<String>,
    /// Told to make the command stop.
    stop: Arc<Notify>,
}

impl CommandRun {
    fn to_json(&self) -> Value {
        json!({ "running": self.running, "code": self.code, "output": self.output })
    }
}

struct Hub {
    sessions: Vec<Session>,
    store: Store,
    /// What every connected UI is sent, as it happens.
    to_ui: broadcast::Sender<String>,
    /// The address tests started from here are told to report to. Without it, nothing is
    /// started from here.
    run_address: Option<String>,
    commands: HashMap<String, CommandRun>,
    /// When the UIs were last told what the batches know, and whether more is known since.
    batches_told: Instant,
    batches_changed: bool,
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
        let mut batches = json!(or_complain(self.store.batches()));
        for batch in batches.as_array_mut().into_iter().flatten() {
            let running = batch["name"]
                .as_str()
                .and_then(|name| self.commands.get(name))
                .is_some_and(|command| command.running);
            batch["running"] = json!(running);
        }
        batches
    }

    fn tell_batches(&mut self) {
        self.batches_told = Instant::now();
        self.batches_changed = false;
        self.tell_ui(json!({ "kind": "batches", "batches": self.batches() }));
    }

    /// Tells the UIs what the batches know, unless they were told a moment ago: tests report
    /// faster than anybody can read.
    fn tell_batches_soon(&mut self) {
        if self.batches_told.elapsed() >= BATCHES_TOLD_EVERY {
            self.tell_batches();
        } else {
            self.batches_changed = true;
        }
    }

    fn snapshot(&self) -> Value {
        let sessions: Vec<Value> = self.sessions.iter().map(Session::to_json).collect();
        json!({
            "kind": "snapshot",
            "sessions": sessions,
            "batches": self.batches(),
            "can_run": self.run_address.is_some(),
        })
    }

    fn opened(&mut self, session: Session, sources: &Sources) {
        or_complain(self.store.binary_reports(
            &session.batch,
            &session.binary,
            &session.announced,
            sources,
            session.workspace_dir.as_deref(),
        ));
        self.tell_ui(json!({ "kind": "session_opened", "session": session.to_json() }));
        self.sessions.push(session);
        self.tell_batches();
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
                if select["kind"] == "all" {
                    let session = self.session(id);
                    let (batch, binary) = (session.batch.clone(), session.binary.clone());
                    or_complain(self.store.tree_starts_over(&batch, &binary, &root));
                    self.tell_batches_soon();
                }
                answer = Some(json!({ "kind": "run", "root": event["root"], "select": select }));
            }
            // What a test said goes to the batch as soon as it is known.
            Some("test") => {
                let session = self
                    .sessions
                    .iter_mut()
                    .find(|session| session.id == id)
                    .expect("the session of a test that reports is there");
                let tree = session.running.entry(root).or_default();
                let known = tree.apply(&event).and_then(|path| tree.test(&path));
                if let Some(outcome) = known {
                    or_complain(
                        self.store
                            .save_test(&session.batch, &session.binary, &outcome),
                    );
                    self.tell_batches_soon();
                }
            }
            Some("tree_finished") => {
                self.session(id).running.remove(&root);
                self.tell_batches();
            }
            _ => {}
        }
        self.session(id).events.push(event);
        answer
    }

    /// What the tree `root` of a session runs: as the run says, or else as its batch has it.
    fn select(&mut self, id: u64, root: &str) -> Value {
        let session = self
            .sessions
            .iter()
            .find(|session| session.id == id)
            .expect("the session of a tree that is ready is there");
        let (mode, selected) = or_complain(self.store.run(&session.batch).map(Some))
            .unwrap_or((RunMode::All, Vec::new()));
        let in_tree = |path: &TestPath| path.first().is_some_and(|name| name == root);
        let paths: Vec<TestPath> = match session.run.unwrap_or(mode) {
            RunMode::All => return json!({ "kind": "all" }),
            RunMode::Selected => selected
                .into_iter()
                .filter(|test| test.binary == session.binary)
                .map(|test| test.path)
                .filter(in_tree)
                .collect(),
            RunMode::NewAndFailed => {
                let known = or_complain(self.store.statuses(&session.batch, &session.binary, root));
                if known.values().all(|(status, _)| *status == Status::NotRun) {
                    // All of the tree is new.
                    return json!({ "kind": "all" });
                }
                let tests = || session.announced.iter().map(|test| &test.path);
                let wanted: Vec<&TestPath> = tests()
                    .filter(|path| in_tree(path))
                    .filter(|path| known.get(*path).map(|known| known.0) != Some(Status::Passed))
                    .collect();
                // A test that failed only when it was executed again shows that failure
                // when the tests below it run.
                let with_all_below: HashSet<&TestPath> = wanted
                    .iter()
                    .filter(|path| known.get(**path).is_some_and(|known| known.1))
                    .copied()
                    .collect();
                tests()
                    .filter(|path| {
                        wanted.contains(path)
                            || with_all_below.iter().any(|above| path.starts_with(above))
                    })
                    .cloned()
                    .collect()
            }
        };
        if paths.is_empty() {
            json!({ "kind": "none" })
        } else {
            json!({ "kind": "paths", "paths": paths })
        }
    }

    fn closed(&mut self, id: u64) {
        let session = self.session(id);
        session.running.clear();
        session.open = false;
        self.tell_ui(json!({ "kind": "session_closed", "session": id }));
        if self.batches_changed {
            self.tell_batches();
        }
    }

    /// Does what a UI asked for, and returns what only that UI is to be told.
    fn asked(&mut self, command: FromUi) -> Option<Value> {
        match command {
            FromUi::Clear => {
                self.sessions.retain(|session| session.open);
                self.tell_ui(self.snapshot());
            }
            FromUi::GetBatch { name } => {
                let mut batch = json!(or_complain(self.store.batch(&name)));
                if !batch.is_null() {
                    batch["run"] = match self.commands.get(&name) {
                        Some(command) => command.to_json(),
                        None => CommandRun::default().to_json(),
                    };
                }
                return Some(json!({ "kind": "batch", "name": name, "batch": batch }));
            }
            FromUi::DeleteBatch { name } => {
                or_complain(self.store.delete(&name));
                self.tell_batches();
            }
            FromUi::DeleteBinary { batch, binary } => {
                or_complain(self.store.delete_binary(&batch, &binary));
                self.tell_batches();
            }
            FromUi::SetRun {
                batch,
                mode,
                selected,
            } => {
                or_complain(self.store.set_run(&batch, mode, selected.as_deref()));
                self.tell_batches();
            }
            FromUi::SetCommand {
                batch,
                command,
                dir,
            } => {
                or_complain(self.store.set_command(&batch, &command, &dir));
                self.tell_batches();
            }
            FromUi::Diff { from, to } => {
                let diff = or_complain(self.store.diff(&from, &to));
                let (changes, times) = match diff {
                    Some(diff) => (json!(diff.changes), json!(diff.times)),
                    None => (Value::Null, Value::Null),
                };
                return Some(json!({
                    "kind": "diff", "from": from, "to": to, "changes": changes, "times": times,
                }));
            }
            // These need more than the hub: see `Frontend::asked`.
            FromUi::RunBatch { .. } | FromUi::StopBatch { .. } => {}
        }
        None
    }
}

/// What is known of the sources in a directory, and for how long that is believed.
type SourcesKnown = Arc<Mutex<HashMap<String, (Instant, Sources)>>>;

#[derive(Clone)]
pub struct Frontend {
    hub: Arc<Mutex<Hub>>,
    sources: SourcesKnown,
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
                run_address: None,
                commands: HashMap::new(),
                batches_told: Instant::now(),
                batches_changed: false,
            })),
            sources: Default::default(),
        }
    }

    /// Lets the UI run the tests of a batch: the command of the batch is executed on this
    /// machine, and its tests are told to report to `address`, where this frontend listens.
    ///
    /// Whoever reaches the UI can then run any command here. That is why it has to be
    /// allowed, and should only be where nobody else reaches the frontend.
    pub fn allow_commands(self, address: impl Into<String>) -> Self {
        self.hub.lock().unwrap().run_address = Some(address.into());
        self
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

    /// Where the workspace of a test binary is, and the state of the sources there, if the
    /// tests are on this machine.
    async fn sources(&self, hello: &Value) -> (Option<String>, Sources) {
        let Some(manifest_dir) = hello["manifest_dir"].as_str() else {
            return (None, Sources::default());
        };
        let Some(workspace) = command::workspace_dir(Path::new(manifest_dir)) else {
            return (None, Sources::default());
        };
        // The test binaries of one `cargo test` report one after the other: asking git
        // once is enough for all of them.
        let known = self.sources.lock().unwrap().get(&workspace).cloned();
        let sources = match known {
            Some((at, sources)) if at.elapsed() < Duration::from_secs(5) => sources,
            _ => {
                let dir = workspace.clone();
                let sources = tokio::task::spawn_blocking(move || command::sources(&dir))
                    .await
                    .unwrap_or_default();
                let mut known = self.sources.lock().unwrap();
                known.insert(workspace.clone(), (Instant::now(), sources.clone()));
                sources
            }
        };
        (Some(workspace), sources)
    }

    /// Does what a UI asked for, and returns what only that UI is to be told.
    fn asked(&self, asked: FromUi) -> Option<Value> {
        match asked {
            FromUi::RunBatch { batch } => {
                command::run(self, batch);
                None
            }
            FromUi::StopBatch { batch } => {
                if let Some(command) = self.hub.lock().unwrap().commands.get(&batch) {
                    command.stop.notify_one();
                }
                None
            }
            asked => self.hub.lock().unwrap().asked(asked),
        }
    }
}

async fn tests_connected(State(frontend): State<Frontend>, upgrade: WebSocketUpgrade) -> Response {
    upgrade.on_upgrade(move |socket| tests_session(frontend, socket))
}

/// Whether a page from `origin` may talk to the frontend as its UI.
///
/// A browser lets any page open a WebSocket to any address, and the UI may delete batches
/// and, where allowed, run commands. So only the pages of the frontend itself are let in:
/// the ones served from the address the request came to, the ones of this machine (the dev
/// server of the UI), and the window of the desktop app. Something that is not a browser
/// sends no origin, and is let in.
fn is_own_page(origin: Option<&str>, host: Option<&str>) -> bool {
    let Some(origin) = origin else { return true };
    let Some((scheme, address)) = origin.split_once("://") else {
        return false;
    };
    if scheme == "tauri" || Some(address) == host {
        return true;
    }
    let name = match address.rsplit_once(':') {
        Some((name, port)) if port.bytes().all(|byte| byte.is_ascii_digit()) => name,
        _ => address,
    };
    matches!(
        name,
        "localhost" | "127.0.0.1" | "[::1]" | "tauri.localhost"
    )
}

async fn ui_connected(
    State(frontend): State<Frontend>,
    headers: HeaderMap,
    upgrade: WebSocketUpgrade,
) -> Response {
    let text = |name| headers.get(name).and_then(|value| value.to_str().ok());
    if !is_own_page(text(header::ORIGIN), text(header::HOST)) {
        return (StatusCode::FORBIDDEN, "not a page of this frontend").into_response();
    }
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
    let (workspace_dir, sources) = frontend.sources(&hello).await;
    let id = {
        let mut hub = frontend.hub.lock().unwrap();
        let id = hub.sessions.last().map_or(1, |session| session.id + 1);
        hub.opened(Session::new(id, hello, workspace_dir), &sources);
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
                    let Ok(asked) = serde_json::from_str::<FromUi>(text.as_str()) else {
                        continue;
                    };
                    if let Some(reply) = frontend.asked(asked)
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

#[cfg(test)]
mod tests {
    use super::is_own_page;

    #[test]
    fn only_the_pages_of_the_frontend_may_be_its_ui() {
        let host = Some("192.168.1.20:7357");
        // Served by the frontend itself, wherever it listens.
        assert!(is_own_page(Some("http://192.168.1.20:7357"), host));
        // The dev server of the UI, and the window of the desktop app.
        assert!(is_own_page(Some("http://localhost:5173"), host));
        assert!(is_own_page(Some("http://127.0.0.1:7357"), host));
        assert!(is_own_page(Some("tauri://localhost"), host));
        assert!(is_own_page(Some("http://tauri.localhost"), host));
        // Not a browser.
        assert!(is_own_page(None, host));
        // Any other page, also one that only looks like this machine.
        assert!(!is_own_page(Some("https://example.com"), host));
        assert!(!is_own_page(Some("http://localhost.example.com"), host));
        assert!(!is_own_page(Some("null"), host));
    }
}

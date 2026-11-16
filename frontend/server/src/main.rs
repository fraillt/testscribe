//! The frontend without a window: test binaries report to it, and a browser shows the UI.
//!
//! ```text
//! testscribe-frontend [--listen 127.0.0.1:7357] [--ui <dir of the built UI>] [--db <file>]
//! testscribe-frontend diff <batch> <batch> [--db <file>]
//! ```

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use testscribe_frontend_server::outcome::title;
use testscribe_frontend_server::store::{ChangeKind, Store};
use testscribe_frontend_server::{DEFAULT_PORT, Frontend};
use tokio::net::TcpListener;

/// Where the named batches are stored, unless told otherwise: in the directory the frontend
/// is started in.
const DEFAULT_DATABASE: &str = "testscribe-batches.sqlite";

const USAGE: &str =
    "usage: testscribe-frontend [--listen <address:port>] [--ui <dir>] [--db <file>]
       testscribe-frontend diff <batch> <batch> [--db <file>]";

#[tokio::main]
async fn main() -> ExitCode {
    let mut listen = format!("127.0.0.1:{DEFAULT_PORT}");
    let mut ui_dir = None;
    let mut database = PathBuf::from(DEFAULT_DATABASE);
    let mut words = Vec::new();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if !arg.starts_with("--") {
            words.push(arg);
            continue;
        }
        match (arg.as_str(), args.next()) {
            ("--listen", Some(address)) => listen = address,
            ("--ui", Some(dir)) => ui_dir = Some(PathBuf::from(dir)),
            ("--db", Some(file)) => database = PathBuf::from(file),
            _ => {
                eprintln!("{USAGE}");
                return ExitCode::FAILURE;
            }
        }
    }
    match words.iter().map(String::as_str).collect::<Vec<_>>()[..] {
        [] => {}
        ["diff", from, to] => return diff(&database, from, to),
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::FAILURE;
        }
    }

    // Next to the sources, the built UI is found without being told.
    let ui_dir = ui_dir.or_else(|| {
        let built = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../ui/dist");
        built.join("index.html").exists().then_some(built)
    });

    let frontend = match Frontend::with_database(&database) {
        Ok(frontend) => frontend,
        Err(err) => {
            eprintln!("cannot open the database {}: {err}", database.display());
            return ExitCode::FAILURE;
        }
    };
    let listener = match TcpListener::bind(&listen).await {
        Ok(listener) => listener,
        Err(err) => {
            eprintln!("cannot listen on {listen}: {err}");
            return ExitCode::FAILURE;
        }
    };
    println!("tests:   TESTSCRIBE_FRONTEND={listen} cargo test");
    println!("         with TESTSCRIBE_BATCH_ID=<name>, the results are stored as that batch");
    println!("batches: {}", database.display());
    match &ui_dir {
        Some(dir) => println!("ui:      http://{listen}  (from {})", dir.display()),
        None => println!("ui:      not built; run `npm run build` in frontend/ui"),
    }
    match frontend.serve(listener, ui_dir).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("{err}");
            ExitCode::FAILURE
        }
    }
}

/// Prints what is different in batch `to`, seen from batch `from`. Fails if anything is.
fn diff(database: &Path, from: &str, to: &str) -> ExitCode {
    if !database.exists() {
        eprintln!("there is no database {}", database.display());
        return ExitCode::FAILURE;
    }
    let changes = match Store::open(Some(database)).and_then(|store| store.diff(from, to)) {
        Ok(Some(changes)) => changes,
        Ok(None) => {
            eprintln!(
                "`{from}` and `{to}` are not both batches stored in {}",
                database.display()
            );
            return ExitCode::FAILURE;
        }
        Err(err) => {
            eprintln!("cannot read the database {}: {err}", database.display());
            return ExitCode::FAILURE;
        }
    };
    let count = |kind| changes.iter().filter(|change| change.kind == kind).count();
    for change in &changes {
        let kind = match change.kind {
            ChangeKind::Added => "added",
            ChangeKind::Deleted => "deleted",
            ChangeKind::Modified => "modified",
        };
        let names: Vec<String> = change.path.iter().map(|name| title(name)).collect();
        println!("{kind}: {} > {}", change.binary, names.join(" > "));
        for line in &change.lines {
            println!("  {} {}", line.side, line.text);
        }
        println!();
    }
    println!(
        "{from} -> {to}: {} added, {} deleted, {} modified",
        count(ChangeKind::Added),
        count(ChangeKind::Deleted),
        count(ChangeKind::Modified)
    );
    if changes.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

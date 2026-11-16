//! The frontend without a window: test binaries report to it, and a browser shows the UI.
//!
//! ```text
//! testscribe-frontend [--listen 127.0.0.1:7357] [--ui <dir of the built UI>] [--db <file>]
//! testscribe-frontend diff <batch or file> <batch or file> [--db <file>]
//! testscribe-frontend export <batch> [<file>] [--db <file>]
//! testscribe-frontend import <file> <batch> [--db <file>]
//! ```

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use testscribe_frontend_server::outcome::title;
use testscribe_frontend_server::store::{
    Change, ChangeKind, Export, Store, StoredTest, diff_tests,
};
use testscribe_frontend_server::{DEFAULT_PORT, Frontend};
use tokio::net::TcpListener;

/// Where the named batches are stored, unless told otherwise: in the directory the frontend
/// is started in.
const DEFAULT_DATABASE: &str = "testscribe-batches.sqlite";

const USAGE: &str = "usage: testscribe-frontend [--listen <address:port>] [--ui <dir>] [--db <file>] [--allow-run | --no-run]
       testscribe-frontend diff <batch or file> <batch or file> [--db <file>]
       testscribe-frontend export <batch> [<file>] [--db <file>]
       testscribe-frontend import <file> <batch> [--db <file>]";

/// Whether the address is one only this machine reaches.
fn is_loopback(listen: &str) -> bool {
    let host = listen.rsplit_once(':').map_or(listen, |(host, _port)| host);
    matches!(host, "localhost" | "127.0.0.1" | "[::1]")
}

#[tokio::main]
async fn main() -> ExitCode {
    let mut listen = format!("127.0.0.1:{DEFAULT_PORT}");
    let mut ui_dir = None;
    let mut database = PathBuf::from(DEFAULT_DATABASE);
    let mut allow_run = None;
    let mut words = Vec::new();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if !arg.starts_with("--") {
            words.push(arg);
            continue;
        }
        if arg == "--allow-run" || arg == "--no-run" {
            allow_run = Some(arg == "--allow-run");
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
    let done = match words.iter().map(String::as_str).collect::<Vec<_>>()[..] {
        [] => None,
        ["diff", from, to] => Some(diff(&database, from, to)),
        ["export", batch] => Some(export(&database, batch, None)),
        ["export", batch, file] => Some(export(&database, batch, Some(file))),
        ["import", file, batch] => Some(import(&database, file, batch)),
        _ => Some(Err(USAGE.to_string())),
    };
    match done {
        None => {}
        Some(Ok(code)) => return code,
        Some(Err(why)) => {
            eprintln!("{why}");
            return ExitCode::FAILURE;
        }
    }

    // Next to the sources, the built UI is found without being told.
    let ui_dir = ui_dir.or_else(|| {
        let built = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../ui/dist");
        built.join("index.html").exists().then_some(built)
    });

    // Whoever reaches the UI can run commands here, if that is allowed. Unless told
    // otherwise, it is allowed when only this machine reaches the UI.
    let allow_run = allow_run.unwrap_or_else(|| is_loopback(&listen));
    let frontend = match Frontend::with_database(&database) {
        Ok(frontend) if allow_run => frontend.allow_commands(listen.clone()),
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
    println!(
        "running: {}",
        if allow_run {
            "the UI may run the tests of a batch on this machine (--no-run to forbid it)"
        } else {
            "the UI may not run tests here, as other machines reach it (--allow-run to allow it)"
        }
    );
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

/// The tests a word on the command line names: the ones of a file that was exported, if
/// there is such a file, or else the ones of the batch of that name.
fn tests_of(store: &Store, name: &str) -> Result<Vec<StoredTest>, String> {
    if Path::new(name).is_file() {
        let file = std::fs::read_to_string(name).map_err(|err| format!("{name}: {err}"))?;
        let export: Export = serde_json::from_str(&file)
            .map_err(|err| format!("{name} is not an exported batch: {err}"))?;
        return Ok(export.into_tests());
    }
    match store.batch(name) {
        Ok(Some(batch)) => Ok(batch.tests),
        Ok(None) => Err(format!("there is no file and no stored batch `{name}`")),
        Err(err) => Err(format!("cannot read the database: {err}")),
    }
}

fn open(database: &Path) -> Result<Store, String> {
    Store::open(Some(database))
        .map_err(|err| format!("cannot open the database {}: {err}", database.display()))
}

/// Prints what is different in `to`, seen from `from`. Fails if anything is.
fn diff(database: &Path, from: &str, to: &str) -> Result<ExitCode, String> {
    let store = open(database)?;
    let diff = diff_tests(tests_of(&store, from)?, tests_of(&store, to)?);
    let count = |kind| {
        let of_kind = |change: &&Change| change.kind == kind;
        diff.changes.iter().filter(of_kind).count()
    };
    for change in &diff.changes {
        let kind = match change.kind {
            ChangeKind::Added => "added",
            ChangeKind::Deleted => "deleted",
            ChangeKind::Modified => "modified",
            ChangeKind::NotRun => "not run",
        };
        let names: Vec<String> = change.path.iter().map(|name| title(name)).collect();
        println!("{kind}: {} > {}", change.binary, names.join(" > "));
        for line in &change.lines {
            println!("  {} {}", line.side, line.text);
        }
        println!();
    }
    println!(
        "{from} -> {to}: {} added, {} deleted, {} modified, {} not run",
        count(ChangeKind::Added),
        count(ChangeKind::Deleted),
        count(ChangeKind::Modified),
        count(ChangeKind::NotRun)
    );
    Ok(if diff.changes.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

/// Writes the batch to `file`, or prints it.
fn export(database: &Path, batch: &str, file: Option<&str>) -> Result<ExitCode, String> {
    let export = open(database)?
        .export(batch)
        .map_err(|err| format!("cannot read the database: {err}"))?
        .ok_or_else(|| format!("there is no stored batch `{batch}`"))?;
    let mut text = serde_json::to_string_pretty(&export).expect("a batch is plain data");
    text.push('\n');
    match file {
        Some(file) => std::fs::write(file, text).map_err(|err| format!("{file}: {err}"))?,
        None => print!("{text}"),
    }
    Ok(ExitCode::SUCCESS)
}

/// Makes the batch what `file` says.
fn import(database: &Path, file: &str, batch: &str) -> Result<ExitCode, String> {
    let text = std::fs::read_to_string(file).map_err(|err| format!("{file}: {err}"))?;
    let export: Export = serde_json::from_str(&text)
        .map_err(|err| format!("{file} is not an exported batch: {err}"))?;
    open(database)?
        .import(batch, export)
        .map_err(|err| format!("cannot write the database: {err}"))?;
    Ok(ExitCode::SUCCESS)
}

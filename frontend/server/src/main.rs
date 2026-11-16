//! The frontend without a window: test binaries report to it, and a browser shows the UI.
//!
//! ```text
//! testscribe-frontend [--listen 127.0.0.1:7357] [--ui <dir of the built UI>]
//! ```

use std::path::PathBuf;
use std::process::ExitCode;

use testscribe_frontend_server::{DEFAULT_PORT, Frontend};
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> ExitCode {
    let mut listen = format!("127.0.0.1:{DEFAULT_PORT}");
    let mut ui_dir = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match (arg.as_str(), args.next()) {
            ("--listen", Some(address)) => listen = address,
            ("--ui", Some(dir)) => ui_dir = Some(PathBuf::from(dir)),
            _ => {
                eprintln!("usage: testscribe-frontend [--listen <address:port>] [--ui <dir>]");
                return ExitCode::FAILURE;
            }
        }
    }
    // Next to the sources, the built UI is found without being told.
    let ui_dir = ui_dir.or_else(|| {
        let built = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../ui/dist");
        built.join("index.html").exists().then_some(built)
    });

    let listener = match TcpListener::bind(&listen).await {
        Ok(listener) => listener,
        Err(err) => {
            eprintln!("cannot listen on {listen}: {err}");
            return ExitCode::FAILURE;
        }
    };
    println!("tests:  TESTSCRIBE_FRONTEND={listen} cargo test");
    match &ui_dir {
        Some(dir) => println!("ui:     http://{listen}  (from {})", dir.display()),
        None => println!("ui:     not built; run `npm run build` in frontend/ui"),
    }
    match Frontend::new().serve(listener, ui_dir).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("{err}");
            ExitCode::FAILURE
        }
    }
}

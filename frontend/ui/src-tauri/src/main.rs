//! The frontend as a desktop app: it starts the server and opens a window on the UI.
//!
//! The UI is the one a browser gets from the server, and talks to the server the same way,
//! over a WebSocket. All the window is told is where the server is.

// No console window next to the app on Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri::{WebviewUrl, WebviewWindowBuilder};
use testscribe_frontend_server::{DEFAULT_PORT, Frontend};

/// Overrides where the app listens for tests: `address:port`.
const LISTEN_ENV_NAME: &str = "TESTSCRIBE_FRONTEND_LISTEN";

fn main() {
    let listen = std::env::var(LISTEN_ENV_NAME)
        .unwrap_or_else(|_| format!("127.0.0.1:{DEFAULT_PORT}"));

    tauri::Builder::default()
        .setup(move |app| {
            // If the address is taken, a frontend server is most likely running there
            // already, and the window shows what that one knows.
            if let Ok(listener) = std::net::TcpListener::bind(&listen) {
                listener.set_nonblocking(true)?;
                tauri::async_runtime::spawn(async move {
                    let listener = tokio::net::TcpListener::from_std(listener)
                        .expect("a listener the runtime can use");
                    if let Err(err) = Frontend::new().serve(listener, None).await {
                        eprintln!("the frontend server stopped: {err}");
                    }
                });
            }

            WebviewWindowBuilder::new(app, "main", WebviewUrl::default())
                .title("testscribe")
                .inner_size(1280.0, 900.0)
                .initialization_script(format!("window.__TESTSCRIBE_FRONTEND__ = {listen:?};"))
                .build()?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("the desktop app could not start");
}

//! A standalone tree with a frontend named: it says what it has, asks what to run, runs
//! that, and reports every test.

use std::sync::mpsc;

use futures::{SinkExt, StreamExt};
use serde_json::{Value, json};
use testscribe::prelude::*;
use testscribe::{CASES, detached};
use tokio::net::TcpListener;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::Message;

#[testscribe]
fn order_placed() -> u32 {
    17
}

#[testscribe]
fn order_paid(order: Given<OrderPlaced>) {
    then!(*order => order_number).eq(17);
}

/// The frontend leaves this one out. It would fail the tree if it ran.
#[testscribe]
fn order_cancelled(order: Given<OrderPlaced>) {
    then!(*order => order_number).eq(0);
}

/// A frontend that selects the path to `order_paid`, and hands back what it was told.
fn start_frontend() -> (String, mpsc::Receiver<Vec<Value>>) {
    let (address, listening) = mpsc::channel();
    let (told, everything) = mpsc::channel();
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async move {
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            address
                .send(listener.local_addr().unwrap().to_string())
                .unwrap();
            let (stream, _) = listener.accept().await.unwrap();
            let mut socket = accept_async(stream).await.unwrap();
            let mut messages = Vec::new();
            while let Some(Ok(message)) = socket.next().await {
                let Message::Text(text) = message else {
                    continue;
                };
                let message: Value = serde_json::from_str(text.as_str()).unwrap();
                if message["kind"] == "tree_ready" {
                    let run = json!({
                        "kind": "run",
                        "root": message["root"],
                        "select": {
                            "kind": "paths",
                            "paths": [["frontend::OrderPlaced", "frontend::OrderPaid"]],
                        },
                    });
                    socket.send(Message::text(run.to_string())).await.unwrap();
                }
                let finished = message["kind"] == "tree_finished";
                messages.push(message);
                if finished {
                    break;
                }
            }
            told.send(messages).unwrap();
        });
    });
    (listening.recv().unwrap(), everything)
}

#[test]
fn tree_reports_to_the_frontend_and_runs_what_it_is_told_to() {
    let (address, everything) = start_frontend();
    // SAFETY: this is the only test of the binary, so nothing reads the environment meanwhile.
    unsafe { std::env::set_var(detached::FRONTEND_ENV_NAME, address) };

    let result = detached::run_sync(&CASES, module_path!(), "OrderPlaced");

    assert_eq!(result, Ok(()));
    let messages = everything.recv().unwrap();
    let hello = &messages[0];
    assert_eq!(hello["kind"], "hello");
    assert_eq!(hello["trees"][0]["node"]["name"]["name"], "OrderPlaced");
    assert_eq!(hello["trees"][0]["childs"].as_array().unwrap().len(), 2);
    assert_eq!(
        messages[1],
        json!({ "kind": "tree_ready", "root": "frontend::OrderPlaced" })
    );
    assert_eq!(
        messages.last().unwrap(),
        &json!({ "kind": "tree_finished", "root": "frontend::OrderPlaced", "success": true })
    );

    // What happened to each test, in order: its name and the kind of every update.
    let updates: Vec<(&str, &str)> = messages
        .iter()
        .filter(|message| message["kind"] == "test")
        .map(|message| {
            (
                message["test"]["name"].as_str().unwrap(),
                message["update"]["kind"].as_str().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        updates,
        vec![
            ("OrderPlaced", "Started"),
            ("OrderPlaced", "Finished"),
            ("OrderPaid", "Started"),
            ("OrderPaid", "Updated"),
            ("OrderPaid", "Finished"),
            ("OrderCancelled", "Skipped"),
        ]
    );
}

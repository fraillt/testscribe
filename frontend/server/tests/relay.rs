//! A test binary on one side, a UI on the other: what the first says reaches the second, and
//! is kept in the batch of the run.

use futures::{SinkExt, StreamExt};
use serde_json::{Value, json};
use testscribe_frontend_server::Frontend;
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async};

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

async fn next_json(socket: &mut Socket) -> Value {
    loop {
        if let Message::Text(text) = socket.next().await.unwrap().unwrap() {
            return serde_json::from_str(text.as_str()).unwrap();
        }
    }
}

/// The next message of this kind, whatever comes before it.
async fn next_of_kind(socket: &mut Socket, kind: &str) -> Value {
    loop {
        let message = next_json(socket).await;
        if message["kind"] == kind {
            return message;
        }
    }
}

async fn tell(socket: &mut Socket, message: Value) {
    socket
        .send(Message::text(message.to_string()))
        .await
        .unwrap();
}

async fn start_frontend() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap().to_string();
    tokio::spawn(Frontend::new().serve(listener, None));
    address
}

async fn connect(address: &str, to: &str) -> Socket {
    connect_async(format!("ws://{address}/{to}"))
        .await
        .unwrap()
        .0
}

#[tokio::test]
async fn ui_sees_a_run_as_it_happens_and_one_that_is_over() {
    let address = start_frontend().await;
    let mut early_ui = connect(&address, "ui").await;
    assert_eq!(
        next_json(&mut early_ui).await,
        json!({ "kind": "snapshot", "sessions": [], "batches": [], "can_run": false })
    );

    let mut tests = connect(&address, "tests").await;
    let hello = json!({ "kind": "hello", "binary": "orders", "trees": [] });
    tell(&mut tests, hello.clone()).await;
    let ready = json!({ "kind": "tree_ready", "root": "orders::OrderPlaced" });
    tell(&mut tests, ready.clone()).await;

    // The tree that is ready is told to run, all of it.
    assert_eq!(
        next_json(&mut tests).await,
        json!({ "kind": "run", "root": "orders::OrderPlaced", "select": { "kind": "all" } })
    );

    let opened = next_of_kind(&mut early_ui, "session_opened").await;
    assert_eq!(opened["session"]["hello"], hello);
    assert_eq!(opened["session"]["open"], true);
    // A run that names no batch belongs to the one that is not stored.
    assert_eq!(opened["session"]["batch"], "default");
    let session = opened["session"]["id"].clone();
    assert_eq!(
        next_of_kind(&mut early_ui, "event").await,
        json!({ "kind": "event", "session": session, "event": ready })
    );

    tests.close(None).await.unwrap();
    assert_eq!(
        next_of_kind(&mut early_ui, "session_closed").await,
        json!({ "kind": "session_closed", "session": session })
    );

    // A UI that connects when all is over is told the same, in one piece.
    let mut late_ui = connect(&address, "ui").await;
    let snapshot = next_json(&mut late_ui).await;
    assert_eq!(
        snapshot["sessions"],
        json!([{
            "id": session, "batch": "default", "workspace_dir": null,
            "hello": hello, "events": [ready], "open": false,
        }])
    );
    assert_eq!(snapshot["batches"][0]["name"], "default");
    assert_eq!(snapshot["batches"][0]["stored"], false);
}

fn name(name: &str) -> Value {
    json!({ "path": "orders", "name": name })
}

fn tree(test: &str, childs: Value) -> Value {
    json!({ "node": { "name": name(test) }, "childs": childs })
}

/// A test binary with one tree: `OrderPlaced`, and below it `OrderPaid` and `OrderCancelled`.
fn hello(batch: &str) -> Value {
    let childs = json!([
        tree("OrderPaid", json!([])),
        tree("OrderCancelled", json!([]))
    ]);
    json!({
        "kind": "hello",
        "binary": "orders-31acdf8590b2be92",
        "batch": batch,
        "trees": [tree("OrderPlaced", childs)],
    })
}

/// The messages of a test that runs and makes one check, which fails with `failure`.
fn test_ran(test: &str, above: &[&str], line_nr: u32, failure: Option<&str>) -> Vec<Value> {
    let event = |update: Value| json!({ "kind": "test", "root": "orders::OrderPlaced", "test": name(test), "update": update, "elapsed_us": 7 });
    let path: Vec<Value> = above.iter().map(|above| name(above)).collect();
    let outcome = match failure {
        Some(details) => json!({ "kind": "Failure", "details": details }),
        None => json!({ "kind": "Success" }),
    };
    vec![
        event(
            json!({ "kind": "Started", "info": { "path": path, "run_count": 0, "param_info": null } }),
        ),
        event(json!({ "kind": "Updated", "info": {
            "kind": "Verified", "message": "it is as expected", "line_nr": line_nr, "file": "tests/orders.rs", "outcome": outcome,
        } })),
        event(json!({ "kind": "Finished", "panic_message": null })),
    ]
}

/// Runs the tree as a test binary would: asks what to run, reports `ran`, and leaves.
/// Returns what it was told to run.
async fn run_tree(address: &str, batch: &str, ran: Vec<Vec<Value>>) -> Value {
    run_tree_saying(address, hello(batch), ran).await
}

async fn run_tree_saying(address: &str, hello: Value, ran: Vec<Vec<Value>>) -> Value {
    let mut tests = connect(address, "tests").await;
    tell(&mut tests, hello).await;
    tell(
        &mut tests,
        json!({ "kind": "tree_ready", "root": "orders::OrderPlaced" }),
    )
    .await;
    let select = next_json(&mut tests).await["select"].clone();
    for message in ran.into_iter().flatten() {
        tell(&mut tests, message).await;
    }
    tell(
        &mut tests,
        json!({ "kind": "tree_finished", "root": "orders::OrderPlaced", "success": true }),
    )
    .await;
    tests.close(None).await.unwrap();
    select
}

/// The tests of a batch as the UI is told: the name, the status and the text of each.
async fn batch(ui: &mut Socket, name: &str) -> Vec<(String, String, String)> {
    tell(ui, json!({ "kind": "get_batch", "name": name })).await;
    let batch = next_of_kind(ui, "batch").await;
    batch["batch"]["tests"]
        .as_array()
        .unwrap()
        .iter()
        .map(|test| {
            let text = |value: &Value| value.as_str().unwrap().to_string();
            let path = test["path"].as_array().unwrap();
            (
                text(path.last().unwrap()),
                text(&test["status"]),
                text(&test["text"]),
            )
        })
        .collect()
}

/// Waits until the frontend has taken everything of the runs before.
async fn settled(ui: &mut Socket, sessions: usize) {
    for _ in 0..sessions {
        next_of_kind(ui, "session_closed").await;
    }
}

const PLACED: &[&str] = &["OrderPlaced"];

#[tokio::test]
async fn batch_runs_what_it_is_set_to_and_keeps_what_ran() {
    let address = start_frontend().await;
    let mut ui = connect(&address, "ui").await;

    // A batch that is new runs everything.
    let everything = vec![
        test_ran("OrderPlaced", &[], 10, None),
        test_ran("OrderPaid", PLACED, 20, Some("expected: 5\nactual: 6")),
        test_ran("OrderCancelled", PLACED, 30, None),
    ];
    let select = run_tree(&address, "work", everything.clone()).await;
    assert_eq!(select, json!({ "kind": "all" }));
    settled(&mut ui, 1).await;
    let statuses = |tests: Vec<(String, String, String)>| -> Vec<(String, String)> {
        tests
            .into_iter()
            .map(|(name, status, _)| (name, status))
            .collect()
    };
    let told = |list: &[(&str, &str)]| -> Vec<(String, String)> {
        list.iter()
            .map(|(name, status)| (format!("orders::{name}"), status.to_string()))
            .collect()
    };
    assert_eq!(
        statuses(batch(&mut ui, "work").await),
        told(&[
            ("OrderPlaced", "passed"),
            ("OrderPaid", "failed"),
            ("OrderCancelled", "passed")
        ])
    );

    // Set to the new and the failed tests, it runs the one that failed, and only that one
    // is written over.
    tell(
        &mut ui,
        json!({ "kind": "set_run", "batch": "work", "mode": "new_and_failed" }),
    )
    .await;
    next_of_kind(&mut ui, "batches").await;
    let select = run_tree(
        &address,
        "work",
        vec![
            test_ran("OrderPlaced", &[], 10, None),
            test_ran("OrderPaid", PLACED, 20, None),
        ],
    )
    .await;
    assert_eq!(
        select,
        json!({ "kind": "paths", "paths": [["orders::OrderPlaced", "orders::OrderPaid"]] })
    );
    settled(&mut ui, 1).await;
    assert_eq!(
        statuses(batch(&mut ui, "work").await),
        told(&[
            ("OrderPlaced", "passed"),
            ("OrderPaid", "passed"),
            ("OrderCancelled", "passed")
        ])
    );

    // Nothing is new and nothing failed: nothing runs.
    assert_eq!(
        run_tree(&address, "work", vec![]).await,
        json!({ "kind": "none" })
    );
    settled(&mut ui, 1).await;
    assert_eq!(batch(&mut ui, "work").await.len(), 3);

    // Set to the tests picked in the UI, it runs those.
    let picked =
        json!([{ "binary": "orders", "path": ["orders::OrderPlaced", "orders::OrderCancelled"] }]);
    tell(
        &mut ui,
        json!({ "kind": "set_run", "batch": "work", "mode": "selected", "selected": picked }),
    )
    .await;
    next_of_kind(&mut ui, "batches").await;
    assert_eq!(
        run_tree(&address, "work", vec![]).await,
        json!({ "kind": "paths", "paths": [["orders::OrderPlaced", "orders::OrderCancelled"]] })
    );
    settled(&mut ui, 1).await;

    // Another batch is another story: it is new, and runs everything.
    assert_eq!(
        run_tree(&address, "other", vec![]).await,
        json!({ "kind": "all" })
    );
}

#[tokio::test]
async fn batches_are_compared_by_what_their_tests_say() {
    let address = start_frontend().await;
    let mut ui = connect(&address, "ui").await;

    run_tree(
        &address,
        "before",
        vec![
            test_ran("OrderPlaced", &[], 10, None),
            test_ran("OrderPaid", PLACED, 20, None),
        ],
    )
    .await;
    // The same tests after the code was moved around, with one that now fails and a new one.
    run_tree(
        &address,
        "after",
        vec![
            test_ran("OrderPlaced", &[], 110, None),
            test_ran("OrderPaid", PLACED, 120, Some("expected: 5")),
            test_ran("OrderCancelled", PLACED, 130, None),
        ],
    )
    .await;
    settled(&mut ui, 2).await;

    tell(
        &mut ui,
        json!({ "kind": "diff", "from": "before", "to": "after" }),
    )
    .await;
    let diff = next_of_kind(&mut ui, "diff").await;
    let changes: Vec<(&str, &str)> = diff["changes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|change| {
            let path = change["path"].as_array().unwrap();
            (
                path.last().unwrap().as_str().unwrap(),
                change["kind"].as_str().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        changes,
        vec![
            ("orders::OrderCancelled", "added"),
            ("orders::OrderPaid", "modified")
        ]
    );
    assert_eq!(
        diff["changes"][1]["lines"],
        json!([
            { "side": " ", "text": "When order paid" },
            { "side": "-", "text": "  Then it is as expected" },
            { "side": "+", "text": "✗ Then it is as expected" },
            { "side": "+", "text": "      expected: 5" },
        ])
    );

    // A batch that is deleted is gone.
    tell(&mut ui, json!({ "kind": "delete_batch", "name": "before" })).await;
    let batches = next_of_kind(&mut ui, "batches").await;
    assert_eq!(batches["batches"].as_array().unwrap().len(), 1);
    assert_eq!(batches["batches"][0]["name"], "after");
}

#[tokio::test]
async fn run_says_what_to_run_before_its_batch_does() {
    let address = start_frontend().await;
    let mut ui = connect(&address, "ui").await;
    let everything = vec![
        test_ran("OrderPlaced", &[], 10, None),
        test_ran("OrderPaid", PLACED, 20, Some("expected: 5")),
        test_ran("OrderCancelled", PLACED, 30, None),
    ];
    run_tree(&address, "work", everything).await;
    settled(&mut ui, 1).await;

    // The batch is set to everything, and the run asks for the new and the failed tests.
    let mut saying = hello("work");
    saying["run"] = json!("new-and-failed");
    assert_eq!(
        run_tree_saying(&address, saying, vec![]).await,
        json!({ "kind": "paths", "paths": [["orders::OrderPlaced", "orders::OrderPaid"]] })
    );
    // The other way round.
    tell(
        &mut ui,
        json!({ "kind": "set_run", "batch": "work", "mode": "new_and_failed" }),
    )
    .await;
    next_of_kind(&mut ui, "batches").await;
    let mut saying = hello("work");
    saying["run"] = json!("all");
    assert_eq!(
        run_tree_saying(&address, saying, vec![]).await,
        json!({ "kind": "all" })
    );
}

#[tokio::test]
async fn batch_knows_what_a_test_said_while_its_tree_still_runs() {
    let address = start_frontend().await;
    let mut ui = connect(&address, "ui").await;
    let mut tests = connect(&address, "tests").await;
    tell(&mut tests, hello("work")).await;
    tell(
        &mut tests,
        json!({ "kind": "tree_ready", "root": "orders::OrderPlaced" }),
    )
    .await;
    next_json(&mut tests).await;
    for message in test_ran("OrderPlaced", &[], 10, None) {
        tell(&mut tests, message).await;
    }
    // Asking what to run again is answered after everything before it was taken in.
    tell(
        &mut tests,
        json!({ "kind": "tree_ready", "root": "orders::Other" }),
    )
    .await;
    next_json(&mut tests).await;

    let known: Vec<(String, String)> = batch(&mut ui, "work")
        .await
        .into_iter()
        .map(|(name, status, _)| (name, status))
        .collect();
    assert_eq!(
        known,
        vec![
            ("orders::OrderPlaced".to_string(), "passed".to_string()),
            ("orders::OrderPaid".to_string(), "not_run".to_string()),
            ("orders::OrderCancelled".to_string(), "not_run".to_string()),
        ]
    );
}

#[tokio::test]
async fn test_that_failed_when_executed_again_is_run_with_the_tests_below_it() {
    let address = start_frontend().await;
    let mut ui = connect(&address, "ui").await;
    // `OrderPlaced` passes, and panics when it is executed again for the second test below.
    let again = |update: Value| json!({ "kind": "test", "root": "orders::OrderPlaced", "test": name("OrderPlaced"), "update": update, "elapsed_us": 9 });
    let ran = vec![
        test_ran("OrderPlaced", &[], 10, None),
        test_ran("OrderPaid", PLACED, 20, None),
        vec![
            again(
                json!({ "kind": "Started", "info": { "path": [], "run_count": 1, "param_info": null } }),
            ),
            again(json!({ "kind": "Finished", "panic_message": "no connection" })),
        ],
    ];
    run_tree(&address, "work", ran).await;
    settled(&mut ui, 1).await;

    let mut saying = hello("work");
    saying["run"] = json!("new-and-failed");
    // `OrderPaid` passed, and runs again all the same: alone, `OrderPlaced` would pass.
    assert_eq!(
        run_tree_saying(&address, saying, vec![]).await,
        json!({ "kind": "paths", "paths": [
            ["orders::OrderPlaced"],
            ["orders::OrderPlaced", "orders::OrderPaid"],
            ["orders::OrderPlaced", "orders::OrderCancelled"],
        ] })
    );
}

#[tokio::test]
async fn ui_runs_the_command_of_a_batch_where_that_is_allowed() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap().to_string();
    tokio::spawn(
        Frontend::new()
            .allow_commands(address.clone())
            .serve(listener, None),
    );
    let mut ui = connect(&address, "ui").await;
    assert_eq!(next_json(&mut ui).await["can_run"], true);

    let dir = std::env::temp_dir();
    let command = "echo to $TESTSCRIBE_FRONTEND as $TESTSCRIBE_BATCH_ID; exit 3";
    tell(
        &mut ui,
        json!({ "kind": "set_command", "batch": "work", "command": command, "dir": dir }),
    )
    .await;
    tell(&mut ui, json!({ "kind": "run_batch", "batch": "work" })).await;

    let started = next_of_kind(&mut ui, "command_state").await;
    assert_eq!(started["running"], true);
    let finished = next_of_kind(&mut ui, "command_state").await;
    assert_eq!(
        finished,
        json!({ "kind": "command_state", "batch": "work", "running": false, "code": 3 })
    );

    tell(&mut ui, json!({ "kind": "get_batch", "name": "work" })).await;
    let batch = next_of_kind(&mut ui, "batch").await;
    let output = batch["batch"]["run"]["output"].as_array().unwrap();
    assert_eq!(output[1], json!(format!("to {address} as work")));
    assert_eq!(output[2], "finished with exit code 3");
}

#[tokio::test]
async fn nothing_is_run_where_it_is_not_allowed_and_no_other_page_is_let_in() {
    let address = start_frontend().await;
    let mut ui = connect(&address, "ui").await;
    assert_eq!(next_json(&mut ui).await["can_run"], false);
    tell(
        &mut ui,
        json!({ "kind": "set_command", "batch": "work", "command": "echo hello", "dir": "/" }),
    )
    .await;
    tell(&mut ui, json!({ "kind": "run_batch", "batch": "work" })).await;
    tell(&mut ui, json!({ "kind": "get_batch", "name": "work" })).await;
    let batch = next_of_kind(&mut ui, "batch").await;
    assert_eq!(batch["batch"]["run"]["output"], json!([]));

    // A page of some other site that opens a WebSocket to the frontend.
    use tokio_tungstenite::tungstenite::client::IntoClientRequest;
    let mut request = format!("ws://{address}/ui").into_client_request().unwrap();
    request
        .headers_mut()
        .insert("origin", "https://example.com".parse().unwrap());
    assert!(connect_async(request).await.is_err());
}

#[tokio::test]
async fn command_that_runs_is_stopped_with_everything_it_started() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap().to_string();
    tokio::spawn(
        Frontend::new()
            .allow_commands(address.clone())
            .serve(listener, None),
    );
    let mut ui = connect(&address, "ui").await;
    // The shell starts another process and waits for it, as `cargo test` does.
    let command = "echo started; sleep 60; echo not reached";
    tell(
        &mut ui,
        json!({ "kind": "set_command", "batch": "work", "command": command, "dir": std::env::temp_dir() }),
    )
    .await;
    tell(&mut ui, json!({ "kind": "run_batch", "batch": "work" })).await;
    loop {
        let printed = next_of_kind(&mut ui, "command_output").await;
        if printed["lines"][0] == "started" {
            break;
        }
    }
    let asked = std::time::Instant::now();
    tell(&mut ui, json!({ "kind": "stop_batch", "batch": "work" })).await;
    loop {
        let state = next_of_kind(&mut ui, "command_state").await;
        if state["running"] == false {
            assert_eq!(state["code"], Value::Null);
            break;
        }
    }
    assert!(asked.elapsed() < std::time::Duration::from_secs(10));
}

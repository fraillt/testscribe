//! A test binary on one side, a UI on the other: what the first says reaches the second.

use futures::{SinkExt, StreamExt};
use serde_json::{Value, json};
use testscribe_frontend_server::Frontend;
use tokio::net::TcpListener;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

async fn next_json<S>(socket: &mut S) -> Value
where
    S: StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin,
{
    loop {
        if let Message::Text(text) = socket.next().await.unwrap().unwrap() {
            return serde_json::from_str(text.as_str()).unwrap();
        }
    }
}

async fn start_frontend() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap().to_string();
    tokio::spawn(Frontend::new().serve(listener, None));
    address
}

#[tokio::test]
async fn ui_sees_a_run_as_it_happens_and_one_that_is_over() {
    let address = start_frontend().await;
    let (mut early_ui, _) = connect_async(format!("ws://{address}/ui")).await.unwrap();
    assert_eq!(
        next_json(&mut early_ui).await,
        json!({ "kind": "snapshot", "sessions": [] })
    );

    let (mut tests, _) = connect_async(format!("ws://{address}/tests"))
        .await
        .unwrap();
    let hello = json!({ "kind": "hello", "binary": "orders", "trees": [] });
    tests.send(Message::text(hello.to_string())).await.unwrap();
    let ready = json!({ "kind": "tree_ready", "root": "orders::OrderPlaced" });
    tests.send(Message::text(ready.to_string())).await.unwrap();

    // The tree that is ready is told to run, all of it.
    assert_eq!(
        next_json(&mut tests).await,
        json!({ "kind": "run", "root": "orders::OrderPlaced", "select": { "kind": "all" } })
    );

    let opened = next_json(&mut early_ui).await;
    assert_eq!(opened["kind"], "session_opened");
    assert_eq!(opened["session"]["hello"], hello);
    assert_eq!(opened["session"]["open"], true);
    let session = opened["session"]["id"].clone();
    assert_eq!(
        next_json(&mut early_ui).await,
        json!({ "kind": "event", "session": session, "event": ready })
    );

    tests.close(None).await.unwrap();
    assert_eq!(
        next_json(&mut early_ui).await,
        json!({ "kind": "session_closed", "session": session })
    );

    // A UI that connects when all is over is told the same, in one piece.
    let (mut late_ui, _) = connect_async(format!("ws://{address}/ui")).await.unwrap();
    assert_eq!(
        next_json(&mut late_ui).await,
        json!({
            "kind": "snapshot",
            "sessions": [{ "id": session, "hello": hello, "events": [ready], "open": false }]
        })
    );
}

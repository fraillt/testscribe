//! What the emptiness checks report when the value is not what they expect.

use testscribe::report::basic::CheckEmpty;
use testscribe::{CASES, testscribe};

use crate::utils::run::run_tree;

mod utils;

#[testscribe]
fn two_emails_sent() {
    let sent_emails = vec!["welcome", "receipt"];
    then!(sent_emails).is_empty();
}

#[testscribe]
fn no_orders_open() {
    let open_orders: Vec<u32> = Vec::new();
    then!(open_orders).is_not_empty();
}

#[tokio::test]
async fn is_empty_fails_for_a_value_with_something_in_it_and_shows_it() {
    let run = run_tree(&CASES, "TwoEmailsSent").await;

    assert_eq!(
        run.failed(),
        vec![("TwoEmailsSent", vec![r#"actual: ["welcome", "receipt"]"#])]
    );
    assert!(run.output.contains("sent emails is empty"));
}

#[tokio::test]
async fn is_not_empty_fails_for_an_empty_value() {
    let run = run_tree(&CASES, "NoOrdersOpen").await;

    assert_eq!(run.failed(), vec![("NoOrdersOpen", vec!["actual: []"])]);
    assert!(run.output.contains("open orders is not empty"));
}

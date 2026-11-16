//! What the `is_rejected` check reports when the action was not refused the way it expects.

use testscribe::report::basic::CheckRejected;
use testscribe::{CASES, testscribe};

use crate::utils::run::run_tree;

mod utils;

#[testscribe]
fn unpaid_order_paid() {
    let paying: Result<(), String> = Ok(());
    then!(paying).is_rejected("order is already paid");
}

#[testscribe]
fn cancelled_order_paid() {
    let paying: Result<(), String> = Err("order is cancelled".to_string());
    then!(paying).is_rejected("order is already paid");
}

#[tokio::test]
async fn is_rejected_fails_for_an_action_that_was_accepted() {
    let run = run_tree(&CASES, "UnpaidOrderPaid").await;

    assert_eq!(
        run.failed(),
        vec![("UnpaidOrderPaid", vec!["it was accepted"])]
    );
    assert!(
        run.output
            .contains("paying is rejected: order is already paid")
    );
}

#[tokio::test]
async fn is_rejected_fails_for_another_reason_and_shows_it() {
    let run = run_tree(&CASES, "CancelledOrderPaid").await;

    assert_eq!(
        run.failed(),
        vec![("CancelledOrderPaid", vec!["rejected: order is cancelled"])]
    );
}

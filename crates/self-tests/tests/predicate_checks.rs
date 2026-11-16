//! What the predicate checks report when the value is not what they expect.

use testscribe::report::basic::CheckIs;
use testscribe::{CASES, testscribe};

use crate::utils::run::run_tree;

mod utils;

#[derive(Debug)]
#[allow(dead_code)]
enum Step {
    Applied { block: u64 },
    RolledBack { block: u64 },
}

#[testscribe]
fn block_102_applied() {
    let step = Step::Applied { block: 102 };
    then!(step).is("a rollback", |step| matches!(step, Step::RolledBack { .. }));
}

#[testscribe]
fn block_102_rolled_back() {
    let step = Step::RolledBack { block: 102 };
    then!(step).is_not("a rollback", |step| matches!(step, Step::RolledBack { .. }));
}

#[testscribe]
fn step_with_panicking_predicate() {
    let step = Step::Applied { block: 102 };
    then!(step).is("a rollback", |_| panic!("block 102 not found"));
}

#[tokio::test]
async fn is_fails_for_a_value_the_predicate_rejects_and_shows_it() {
    let run = run_tree(&CASES, "Block102Applied").await;

    assert_eq!(
        run.failed(),
        vec![("Block102Applied", vec!["actual: Applied { block: 102 }"])]
    );
    assert!(run.output.contains("step is a rollback"));
}

#[tokio::test]
async fn is_not_fails_for_a_value_the_predicate_accepts_and_shows_it() {
    let run = run_tree(&CASES, "Block102RolledBack").await;

    assert_eq!(
        run.failed(),
        vec![(
            "Block102RolledBack",
            vec!["actual: RolledBack { block: 102 }"]
        )]
    );
    assert!(run.output.contains("step is not a rollback"));
}

#[tokio::test]
async fn is_fails_when_the_predicate_panics_and_carries_the_panic_message() {
    let run = run_tree(&CASES, "StepWithPanickingPredicate").await;

    assert_eq!(
        run.failed(),
        vec![(
            "StepWithPanickingPredicate",
            vec!["panic: block 102 not found"]
        )]
    );
}

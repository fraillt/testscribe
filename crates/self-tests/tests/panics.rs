//! What a tree reports when something in it panics.
//!
//! A tree must fail when something it needs panics outside of a test body: while an environment
//! is created, while state is cloned for the next branch, or when a parent is executed again
//! for a later branch. In all three the tests below it cannot run, and a run in which tests
//! could not run must never look green.

use std::sync::atomic::{AtomicUsize, Ordering};

use testscribe::report::basic::CheckRun;
use testscribe::test_args::{Env, Environment, Given};
use testscribe::{CASES, testscribe};

use crate::utils::run::run_tree;

mod utils;

struct DatabaseThatIsDown;

impl Environment for DatabaseThatIsDown {
    type Base = ();

    async fn create(_base: Self::Base) -> Self {
        panic!("connection refused")
    }
}

#[testscribe]
#[allow(unused_macros)]
fn root_needing_database(_: Env<DatabaseThatIsDown>) {}

#[testscribe]
#[allow(unused_macros)]
fn child_of_root_needing_database(_: Given<RootNeedingDatabase>) {}

#[testscribe]
#[allow(unused_macros)]
fn root_without_environment() {}

#[testscribe]
#[allow(unused_macros)]
fn leaf_needing_database(_: Given<RootWithoutEnvironment>, _: Env<DatabaseThatIsDown>) {}

static SLOW_DATABASE_ATTEMPTS: AtomicUsize = AtomicUsize::new(0);

struct SlowDatabaseThatIsDown;

impl Environment for SlowDatabaseThatIsDown {
    type Base = ();

    async fn create(_base: Self::Base) -> Self {
        SLOW_DATABASE_ATTEMPTS.fetch_add(1, Ordering::SeqCst);
        panic!("connection timed out")
    }
}

#[testscribe]
#[allow(unused_macros)]
fn root_needing_slow_database(_: Env<SlowDatabaseThatIsDown>) {}

#[testscribe]
#[allow(unused_macros)]
fn first_child_of_slow_root(_: Given<RootNeedingSlowDatabase>) {}

#[testscribe]
#[allow(unused_macros)]
fn grandchild_of_slow_root(_: Given<FirstChildOfSlowRoot>) {}

#[testscribe]
#[allow(unused_macros)]
fn second_child_of_slow_root(_: Given<RootNeedingSlowDatabase>) {}

#[testscribe]
#[allow(unused_macros)]
fn third_child_of_slow_root(_: Given<RootNeedingSlowDatabase>) {}

struct StateThatCannotBeCloned;

impl Clone for StateThatCannotBeCloned {
    fn clone(&self) -> Self {
        panic!("snapshot failed")
    }
}

#[testscribe(cloneable)]
#[allow(unused_macros)]
fn root_with_cloned_state() -> StateThatCannotBeCloned {
    StateThatCannotBeCloned
}

#[testscribe]
#[allow(unused_macros)]
fn first_child_of_cloned_state(_: Given<RootWithClonedState>) {}

#[testscribe]
#[allow(unused_macros)]
fn second_child_of_cloned_state(_: Given<RootWithClonedState>) {}

static ROOT_RUNS: AtomicUsize = AtomicUsize::new(0);

/// Works the first time and panics when it is executed again for its second child.
#[testscribe]
#[allow(unused_macros)]
fn root_failing_on_second_run() {
    if ROOT_RUNS.fetch_add(1, Ordering::SeqCst) > 0 {
        panic!("connection lost")
    }
}

#[testscribe]
#[allow(unused_macros)]
fn first_child_of_flaky_root(_: Given<RootFailingOnSecondRun>) {}

#[testscribe]
#[allow(unused_macros)]
fn second_child_of_flaky_root(_: Given<RootFailingOnSecondRun>) {}

#[testscribe]
#[allow(unused_macros)]
fn root_panicking_in_its_body() {
    panic!("order 17 not found")
}

#[testscribe]
fn root_with_panicking_check() {
    then!("order is found").run(|| -> bool { panic!("order 17 not found") });
}

#[tokio::test]
async fn root_fails_when_its_environment_cannot_be_created() {
    let run = run_tree(&CASES, "RootNeedingDatabase").await;

    assert!(!run.summary.is_success());
    assert_eq!(
        run.failed(),
        vec![(
            "RootNeedingDatabase",
            vec![
                "environment `panics::DatabaseThatIsDown` could not be created: connection refused"
            ]
        )]
    );
    assert_eq!(
        run.narrative(),
        vec![
            "! root needing database",
            "? child of root needing database"
        ]
    );
}

#[tokio::test]
async fn leaf_fails_when_its_environment_cannot_be_created() {
    let run = run_tree(&CASES, "RootWithoutEnvironment").await;

    assert!(!run.summary.is_success());
    assert_eq!(
        run.failed(),
        vec![(
            "LeafNeedingDatabase",
            vec![
                "environment `panics::DatabaseThatIsDown` could not be created: connection refused"
            ]
        )]
    );
    assert_eq!(
        run.narrative(),
        vec!["  root without environment", "! leaf needing database"]
    );
}

#[tokio::test]
async fn test_fails_when_its_state_cannot_be_cloned_for_its_children() {
    let run = run_tree(&CASES, "RootWithClonedState").await;

    assert!(!run.summary.is_success());
    assert_eq!(
        run.failed(),
        vec![(
            "RootWithClonedState",
            vec!["state or environment could not be cloned for the next test: snapshot failed"]
        )]
    );
    assert_eq!(
        run.narrative(),
        vec![
            "  root with cloned state",
            "! root with cloned state",
            "? first child of cloned state",
            "? second child of cloned state"
        ]
    );
}

#[tokio::test]
async fn test_fails_when_it_panics_while_executed_again_for_a_later_child() {
    let run = run_tree(&CASES, "RootFailingOnSecondRun").await;

    assert!(!run.summary.is_success());
    assert_eq!(
        run.failed(),
        vec![("RootFailingOnSecondRun", vec!["connection lost"])]
    );
    assert_eq!(
        run.narrative(),
        vec![
            "  root failing on second run",
            "  first child of flaky root",
            "! root failing on second run",
            "? second child of flaky root"
        ]
    );
}

#[tokio::test]
async fn failure_of_panicking_test_carries_the_panic_message() {
    let run = run_tree(&CASES, "RootPanickingInItsBody").await;

    assert_eq!(
        run.failed(),
        vec![("RootPanickingInItsBody", vec!["order 17 not found"])]
    );
}

#[tokio::test]
async fn failure_of_panicking_check_carries_the_panic_message() {
    let run = run_tree(&CASES, "RootWithPanickingCheck").await;

    assert_eq!(
        run.failed(),
        vec![("RootWithPanickingCheck", vec!["panic: order 17 not found"])]
    );
}

#[tokio::test]
async fn failed_test_is_not_executed_again_for_every_test_below_it() {
    let run = run_tree(&CASES, "RootNeedingSlowDatabase").await;

    assert_eq!(SLOW_DATABASE_ATTEMPTS.load(Ordering::SeqCst), 1);
    assert_eq!(
        run.narrative(),
        vec![
            "! root needing slow database",
            "? first child of slow root",
            "? grandchild of slow root",
            "? second child of slow root",
            "? third child of slow root"
        ]
    );
}

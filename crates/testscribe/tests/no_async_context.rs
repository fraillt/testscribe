mod utils;

use testscribe::report::basic::CheckRun;
use testscribe::tests_tree::BuildDagError;
use testscribe::{CASES, testscribe};
use utils::create_fq_name;
use utils::tree::create_and_verify_tt;

#[testscribe]
async fn async_test() {
    then!("boo").run(|| {});
}

#[test]
fn async_test_must_run_in_async_context() {
    match create_and_verify_tt(&CASES, false).unwrap_err() {
        BuildDagError::AsyncRuntimeRequired { test: test_name } => {
            assert_eq!(test_name, create_fq_name("no_async_context::AsyncTest"));
        }
        err => panic!("Unexpected error: {err}"),
    }
}

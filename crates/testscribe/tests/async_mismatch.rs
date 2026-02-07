mod utils;
use testscribe::report::basic::{CheckAsyncRun, CheckRun};
use testscribe::test_args::Given;
use testscribe::tests_tree::BuildTreeError;
use testscribe::{CASES, testscribe};
use utils::create_fq_name;
use utils::tree::create_and_verify_tt;

#[testscribe]
fn not_async() {
    then!("boo").run(|| {});
}

#[testscribe]
async fn xxx(_: Given<NotAsync>) {
    then!("boo").run_async(|| async {}).await;
}

#[test]
fn async_test_must_run_in_async_context() {
    match create_and_verify_tt(&CASES, false).unwrap_err() {
        BuildTreeError::AsyncnessMismatch {
            parent,
            parent_is_async,
            test,
            test_is_async,
        } => {
            assert_eq!(test, create_fq_name("async_mismatch::Xxx"));
            assert!(test_is_async);
            assert_eq!(parent, create_fq_name("async_mismatch::NotAsync"));
            assert!(!parent_is_async);
        }
        err => panic!("Unexpected error: {err}"),
    }
}

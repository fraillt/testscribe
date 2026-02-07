use testscribe::test_args::{Env, Environment};
use testscribe::test_case::FqFnName;
use testscribe::tests_tree::BuildTreeError;
use testscribe::{CASES, testscribe};

use crate::utils::tree::create_and_verify_tt;

mod utils;

struct NotUsed {}

impl Environment for NotUsed {
    type Base = ();

    async fn create(_base: Self::Base) -> Self {
        todo!()
    }
}

struct EnvRoot {}

impl Environment for EnvRoot {
    type Base = NotUsed;

    async fn create(_base: Self::Base) -> Self {
        todo!()
    }
}

#[testscribe]
fn root_test(_: Env<EnvRoot>) {
    then!("");
}

#[test]
fn boo() {
    let err = create_and_verify_tt(&CASES, false).unwrap_err();
    let BuildTreeError::EnvironmentBaseMismatch {
        current_test,
        env_name,
        expected_base,
        actual_base,
    } = err
    else {
        panic!("Must be environment initialization error");
    };
    assert_eq!(
        current_test,
        FqFnName::new("environment_init_error", "RootTest")
    );
    assert_eq!(env_name, FqFnName::new("environment_init_error", "EnvRoot"));
    assert_eq!(
        expected_base,
        FqFnName::new("environment_init_error", "NotUsed")
    );
    assert_eq!(actual_base, FqFnName::new("", "()"));
}

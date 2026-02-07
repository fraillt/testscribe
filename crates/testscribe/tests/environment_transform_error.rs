use testscribe::test_args::{Env, Environment, Given};
use testscribe::tests_tree::BuildTreeError;
use testscribe::{CASES, testscribe};

use crate::utils::create_fq_name;
use crate::utils::tree::create_and_verify_tt;

mod utils;

struct EnvRoot {}

impl Environment for EnvRoot {
    type Base = ();

    async fn create(_base: Self::Base) -> Self {
        Self {}
    }
}

struct EnvNext {}

impl Environment for EnvNext {
    type Base = ();

    async fn create(_base: Self::Base) -> Self {
        Self {}
    }
}

#[testscribe]
fn root_test(_: Env<EnvRoot>) {
    then!("");
}

#[testscribe]
fn next_test(_: Given<RootTest>, _: Env<EnvNext>) {
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
        panic!("Must be environment transform error");
    };
    assert_eq!(
        current_test,
        create_fq_name("environment_transform_error::NextTest")
    );
    assert_eq!(
        env_name,
        create_fq_name("environment_transform_error::EnvNext")
    );
    assert_eq!(expected_base, create_fq_name("()"));
    assert_eq!(
        actual_base,
        create_fq_name("environment_transform_error::EnvRoot")
    );
}

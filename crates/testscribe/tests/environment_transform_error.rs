use testscribe::test_args::{Env, Given};
use testscribe::tests_tree::BuildDagError;
use testscribe::{CASES, testscribe};

use crate::utils::create_fq_name;
use crate::utils::tree::create_and_verify_tt;

mod utils;
#[testscribe(env)]
fn env_root() -> bool {
    true
}

#[testscribe(env)]
fn env_next(_: String) -> i32 {
    5
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
    let BuildDagError::EnvironmentTransformMismatch {
        current_test,
        current_env_init_type,
        parent_env_type,
    } = err
    else {
        panic!("Must be environment transform error");
    };
    assert_eq!(
        current_test,
        create_fq_name("environment_transform_error::NextTest")
    );
    assert_eq!(current_env_init_type, "alloc::string::String");
    assert_eq!(parent_env_type, "bool");
}

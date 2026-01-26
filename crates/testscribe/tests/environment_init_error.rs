use testscribe::test_args::Env;
use testscribe::test_case::FqFnName;
use testscribe::tests_tree::BuildDagError;
use testscribe::{CASES, testscribe};

use crate::utils::tree::create_and_verify_tt;

mod utils;
#[testscribe(env)]
fn env_root(_: bool) -> bool {
    true
}

#[testscribe]
fn root_test(_: Env<EnvRoot>) {
    then!("");
}

#[test]
fn boo() {
    eprintln!("{:#?}", CASES.iter());
    let x = CASES.iter().next().unwrap().env.as_ref().unwrap();
    eprintln!("{:#?}", (x.get_name)());
    let err = create_and_verify_tt(&CASES, false).unwrap_err();
    let BuildDagError::EnvironmentInitiationWithArgument { env_name } = err else {
        panic!("Must be environment initialization error");
    };
    assert_eq!(env_name, FqFnName::new("environment_init_error", "EnvRoot"));
}

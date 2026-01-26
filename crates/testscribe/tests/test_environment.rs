mod utils;

use testscribe::report::basic::CheckEq;
use testscribe::test_args::{Env, Given};
use testscribe::testscribe;

#[testscribe(env)]
async fn xxx() -> i32 {
    54
}

#[testscribe(env)]
fn xxx2(p: i32) -> bool {
    p == 10
}

#[testscribe]
fn wrap_env_in_another(_: Given<DependsOnUpdatedEnv>, Env(e): Env<Xxx2>) {
    then!(e).eq(&true);
}

#[testscribe]
fn depends_on_updated_env(given: Given<SetEnvTo10>, e: Env<Xxx>) {
    then!(*given => sd).eq(true);
    then!(*e => env).eq(10);
}

#[testscribe]
fn bar(_: Given<SetEnvTo10>, _: Env<Xxx>) {
    then!("");
}

#[testscribe]
fn foo(_: Given<SetEnvTo10>) {
    then!("");
}

#[testscribe(standalone)]
#[test]
fn set_env_to_10(mut e: Env<Xxx>) -> bool {
    *e = 10;
    then!(*e => env).eq(10);
    true
}

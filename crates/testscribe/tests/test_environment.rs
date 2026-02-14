mod utils;

use testscribe::clone_async::CloneAsync;
use testscribe::report::basic::CheckEq;
use testscribe::test_args::{Env, Environment, Given};
use testscribe::testscribe;

#[derive(Debug, Clone)]
struct Initial {
    value: i32,
}

impl Environment for Initial {
    type Base = ();

    async fn create(_base: Self::Base) -> Self {
        Initial { value: 54 }
    }
}

impl CloneAsync for Initial {
    async fn clone_async(&self) -> Self {
        Self { value: self.value }
    }
}

struct NextEnv {
    value: bool,
}

impl Environment for NextEnv {
    type Base = Initial;

    async fn create(base: Self::Base) -> Self {
        NextEnv {
            value: base.value == 10,
        }
    }
}

#[testscribe]
fn wrap_env_in_another(_: Given<DependsOnUpdatedEnv>, Env(e): Env<NextEnv>) {
    then!(e.value => env).eq(true);
}

#[testscribe(cloneable)]
fn depends_on_updated_env(given: Given<SetEnvTo10>, e: Env<Initial>) {
    then!(*given => sd).eq(true);
    then!(e.value => env).eq(10);
}

#[testscribe]
fn bar(_: Given<SetEnvTo10>, _: Env<Initial>) {
    then!("");
}

#[testscribe]
fn foo(_: Given<SetEnvTo10>) {
    then!("");
}

#[testscribe(standalone)]
#[test]
fn set_env_to_10(mut e: Env<Initial>) -> bool {
    e.value = 10;
    then!(e.value => env).eq(10);
    true
}

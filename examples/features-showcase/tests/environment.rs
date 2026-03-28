use testscribe::report::basic::CheckEq;
use testscribe::test_args::{Env, Environment, Given};
use testscribe::testscribe;

#[derive(Debug)]
struct InitialEnv {
    value: i32,
}

// Implement this trait to use an environment in testscribe tests.
impl Environment for InitialEnv {
    /// Environment can evolve/transform while going down the test tree,
    /// so we need to specify the base environment that will be used to create this environment.
    type Base = ();

    /// Whether a test is sync or async, environment creation is always async,
    /// so it can perform async initialization when needed.
    async fn create(_base: Self::Base) -> Self {
        InitialEnv { value: 54 }
    }
}

/// Another environment that will be created based on `InitialEnv`.
struct NextEnv {
    value: bool,
}

impl Environment for NextEnv {
    /// This environment will be created based on `InitialEnv`, so we specify it as base environment.
    type Base = InitialEnv;

    async fn create(_base: Self::Base) -> Self {
        NextEnv { value: true }
    }
}

/// To use an environment in a test, add it as a test function parameter wrapped in `Env`.
/// Running this test will give this outcome:
/// ```text
/// | 5.030μs|Given env initialized
/// |       -|  Then initial_env_value is equal to 54
/// | 1.610μs|  When env transformed
/// |       -|    Then transformed_env_value is equal to true
/// ```
#[testscribe(standalone)]
#[test]
fn env_initialized(e: Env<InitialEnv>) {
    then!(e.value => initial_env_value).eq(54);
}

#[testscribe]
fn env_transformed(_: Given<EnvInitialized>, env: Env<NextEnv>) {
    then!(env.value => transformed_env_value).eq(true);
}

#[testscribe(standalone)]
#[tokio::test]
// Notice that we need to provide explicit lifetime for the `Env` parameter in async test.
async fn async_env_initialized(e: Env<'_, InitialEnv>) {
    then!(e.value => initial_env_value).eq(54);
}

#[allow(unused_macros)]
#[testscribe]
async fn environment_is_dropped_if_not_used(_: Given<AsyncEnvInitialized>) {
    // Environment is dropped before executing this test, because it is not used in the test function
}

// For demonstration purposes some test function doesn't have assertions, so we suppress the unused macro warning.
#![allow(unused_macros)]

use testscribe::{
    clone_async::CloneAsync,
    report::basic::CheckEq,
    test_args::{Env, Environment, Given},
    testscribe,
};

/// This enum demonstrates cloning test state and environment in both sync and async contexts.
#[derive(Debug, PartialEq)]
enum SomeEnv {
    Created,
    ClonedSync,
    ClonedAsync,
}

impl Clone for SomeEnv {
    fn clone(&self) -> Self {
        Self::ClonedSync
    }
}

/// To clone test state (and environment) in async contexts, implement `CloneAsync`.
impl CloneAsync for SomeEnv {
    async fn clone_async(&self) -> Self {
        Self::ClonedAsync
    }
}

impl Environment for SomeEnv {
    type Base = ();

    async fn create(_: Self::Base) -> Self {
        Self::Created
    }
}

/// Add the `cloneable` attribute to clone test state and environment in sync contexts.
/// Running this test will give this outcome:
/// ```text
/// | 3.260μs|Given expensive test state we want to clone
/// | 4.280μs|  When first child test starts
/// |       -|    Then test_state is equal to Created
/// |       -|    And env_state is equal to Created
/// | 1.990μs|  When last child test
/// |       -|    Then test_state is equal to ClonedSync
/// |       -|    And env_state is equal to ClonedSync
///```
#[testscribe(standalone, cloneable)]
#[tokio::test]
async fn expensive_test_state_we_want_to_clone(_: Env<'_, SomeEnv>) -> SomeEnv {
    SomeEnv::Created
}

// You might want to put some test functions in a separate module for a few reasons:
// 1. To group related tests together
// 2. To use same test names, but have different parents

mod clone_sync {
    use super::*;
    /// Children are executed in the order they are defined, so the first child runs before the last.
    /// Before executing the first child, the framework clones and stores the parent state for child execution.
    /// This function still receives the original parent state, not the cloned one,
    /// so we can assert that it is the `Created` variant of `SomeEnv`.
    #[testscribe]
    async fn first_child_test_starts(
        Given(test_state): Given<ExpensiveTestStateWeWantToClone>,
        Env(env_state): Env<'_, SomeEnv>,
    ) {
        then!(test_state).eq(SomeEnv::Created);
        then!(env_state).eq(&SomeEnv::Created);
    }

    /// The parent test is not executed again; the framework reuses the stored cloned state.
    /// The framework also avoids cloning state more than needed.
    #[testscribe]
    async fn last_child_test(
        Given(test_state): Given<ExpensiveTestStateWeWantToClone>,
        Env(env_state): Env<'_, SomeEnv>,
    ) {
        then!(test_state).eq(SomeEnv::ClonedSync);
        then!(env_state).eq(&SomeEnv::ClonedSync);
    }
}

/// If cloning itself requires async context, use the `cloneable_async` attribute.
/// Running this test will give this outcome:
/// ```text
/// | 3.330μs|Given state and env can only be cloned in async context
/// | 4.320μs|  When first child test starts
/// |       -|    Then test_state is equal to Created
/// |       -|    And env_state is equal to Created
/// | 1.690μs|  When last child test
/// |       -|    Then test_state is equal to ClonedAsync
/// |       -|    And env_state is equal to ClonedAsync
///```

#[testscribe(standalone, cloneable_async)]
#[tokio::test]
async fn state_and_env_can_only_be_cloned_in_async_context(_: Env<'_, SomeEnv>) -> SomeEnv {
    SomeEnv::Created
}

mod clone_async {
    use super::*;

    #[testscribe]
    async fn first_child_test_starts(
        Given(test_state): Given<StateAndEnvCanOnlyBeClonedInAsyncContext>,
        Env(env_state): Env<'_, SomeEnv>,
    ) {
        then!(test_state).eq(SomeEnv::Created);
        then!(env_state).eq(&SomeEnv::Created);
    }

    #[testscribe]
    async fn last_child_test(
        Given(test_state): Given<StateAndEnvCanOnlyBeClonedInAsyncContext>,
        Env(env_state): Env<'_, SomeEnv>,
    ) {
        then!(test_state).eq(SomeEnv::ClonedAsync);
        then!(env_state).eq(&SomeEnv::ClonedAsync);
    }
}

// For demonstration purposes some test function doesn't have assertions, so we suppress the unused macro warning.
#![allow(unused_macros)]

use testscribe::prelude::*;

/// This enum demonstrates cloning test state and environment in both sync and async contexts.
#[derive(Debug, PartialEq)]
enum SomeEnv {
    Created,
    ClonedSync(u8),
    ClonedAsync(u8),
}

/// `clone` deliberately returns a *different* variant than the original, purely so cloning is
/// visible in the test output. In real code a clone is an equivalent copy of the original.
impl Clone for SomeEnv {
    fn clone(&self) -> Self {
        match self {
            Self::Created => Self::ClonedSync(1),
            Self::ClonedSync(n) => Self::ClonedSync(*n + 1),
            Self::ClonedAsync(n) => Self::ClonedAsync(*n + 1),
        }
    }
}

/// To clone test state (and environment) in async contexts, implement `CloneAsync`.
impl CloneAsync for SomeEnv {
    async fn clone_async(&self) -> Self {
        match self {
            Self::Created => Self::ClonedAsync(1),
            Self::ClonedSync(n) => Self::ClonedAsync(*n + 1),
            Self::ClonedAsync(n) => Self::ClonedAsync(*n + 1),
        }
    }
}

impl Environment for SomeEnv {
    type Base = ();

    async fn create(_: Self::Base) -> Self {
        Self::Created
    }
}

/// Add the `cloneable` attribute to clone test state and environment in sync contexts.
///
/// A cloneable node runs **once**; its result becomes a read-only template and **every** child
/// branch — including the first — continues from its own independent clone. The template is
/// never handed to a child, so the `Created` value the node returns is never observed by one;
/// each child sees the cloned `ClonedSync` variant instead.
///
/// Running this test will give this outcome:
/// ```text
/// | 3.260μs|Given expensive test state we want to clone
/// | 4.280μs|  When first child test starts
/// |       -|    Then test_state is equal to ClonedSync(1)
/// |       -|    And env_state is equal to ClonedSync(1)
/// | 1.990μs|  When last child test
/// |       -|    Then test_state is equal to ClonedSync(1)
/// |       -|    And env_state is equal to ClonedSync(1)
///```
#[testscribe(standalone(tokio::test), cloneable)]
async fn expensive_test_state_we_want_to_clone(_: Env<SomeEnv>) -> SomeEnv {
    SomeEnv::Created
}

// You might want to put some test functions in a separate module for a few reasons:
// 1. To group related tests together
// 2. To use same test names, but have different parents

mod clone_sync {
    use super::*;
    /// Children run in the order they are defined. The parent node ran once; this first child
    /// receives an independent clone of its stored template, so it observes `ClonedSync` — not
    /// the original `Created`.
    #[testscribe]
    async fn first_child_test_starts(
        Given(test_state): Given<ExpensiveTestStateWeWantToClone>,
        Env(env_state): Env<SomeEnv>,
    ) {
        then!(test_state).eq(SomeEnv::ClonedSync(1));
        then!(env_state).eq(&SomeEnv::ClonedSync(1));
    }

    /// The parent test is not executed again; this sibling continues from its own independent
    /// clone of the same template.
    #[testscribe]
    async fn last_child_test(
        Given(test_state): Given<ExpensiveTestStateWeWantToClone>,
        Env(env_state): Env<SomeEnv>,
    ) {
        then!(test_state).eq(SomeEnv::ClonedSync(1));
        then!(env_state).eq(&SomeEnv::ClonedSync(1));
    }
}

/// If cloning itself requires async context, use the `cloneable_async` attribute.
/// Running this test will give this outcome:
/// ```text
/// | 3.330μs|Given state and env can only be cloned in async context
/// | 4.320μs|  When first child test starts
/// |       -|    Then test_state is equal to ClonedAsync(1)
/// |       -|    And env_state is equal to ClonedAsync(1)
/// | 1.690μs|  When last child test
/// |       -|    Then test_state is equal to ClonedAsync(1)
/// |       -|    And env_state is equal to ClonedAsync(1)
///```
#[testscribe(standalone(tokio::test), cloneable_async)]
async fn state_and_env_can_only_be_cloned_in_async_context(_: Env<SomeEnv>) -> SomeEnv {
    SomeEnv::Created
}

mod clone_async {
    use super::*;

    #[testscribe]
    async fn first_child_test_starts(
        Given(test_state): Given<StateAndEnvCanOnlyBeClonedInAsyncContext>,
        Env(env_state): Env<SomeEnv>,
    ) {
        then!(test_state).eq(SomeEnv::ClonedAsync(1));
        then!(env_state).eq(&SomeEnv::ClonedAsync(1));
    }

    #[testscribe]
    async fn last_child_test(
        Given(test_state): Given<StateAndEnvCanOnlyBeClonedInAsyncContext>,
        Env(env_state): Env<SomeEnv>,
    ) {
        then!(test_state).eq(SomeEnv::ClonedAsync(1));
        then!(env_state).eq(&SomeEnv::ClonedAsync(1));
    }
}

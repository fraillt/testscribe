// For demonstration purposes some test function doesn't have assertions, so we suppress the unused macro warning.
#![allow(unused_macros)]

use testscribe::{
    clone_async::CloneAsync,
    report::basic::CheckEq,
    test_args::{Env, Environment, Given},
    testscribe,
};

#[derive(Debug, PartialEq)]
enum SomeState {
    Created,
    ClonedSync,
    ClonedAsync,
}

impl Clone for SomeState {
    fn clone(&self) -> Self {
        Self::ClonedSync
    }
}

impl CloneAsync for SomeState {
    async fn clone_async(&self) -> Self {
        Self::ClonedAsync
    }
}

impl Environment for SomeState {
    type Base = ();

    async fn create(_: Self::Base) -> Self {
        Self::Created
    }
}

#[testscribe(standalone, cloneable_async)]
#[tokio::test]
async fn async_test_is_cloneable_async(_: Env<'_, SomeState>) -> SomeState {
    SomeState::Created
}

mod clone_async {
    use super::*;
    #[testscribe]
    async fn first_child_test_starts(
        Given(test_state): Given<AsyncTestIsCloneableAsync>,
        Env(env_state): Env<'_, SomeState>,
    ) {
        then!(test_state).eq(SomeState::Created);
        then!(env_state).eq(&SomeState::Created);
    }

    #[testscribe]
    async fn last_child_test(
        Given(test_state): Given<AsyncTestIsCloneableAsync>,
        Env(env_state): Env<'_, SomeState>,
    ) {
        then!(test_state).eq(SomeState::ClonedAsync);
        then!(env_state).eq(&SomeState::ClonedAsync);
    }
}

#[testscribe(standalone, cloneable)]
#[tokio::test]
async fn async_test_is_cloneable(_: Env<'_, SomeState>) -> SomeState {
    SomeState::Created
}

mod clone_sync {
    use super::*;
    #[testscribe]
    async fn first_child_test_starts(
        Given(test_state): Given<AsyncTestIsCloneable>,
        Env(env_state): Env<'_, SomeState>,
    ) {
        then!(test_state).eq(SomeState::Created);
        then!(env_state).eq(&SomeState::Created);
    }

    #[testscribe]
    async fn last_child_test(
        Given(test_state): Given<AsyncTestIsCloneable>,
        Env(env_state): Env<'_, SomeState>,
    ) {
        then!(test_state).eq(SomeState::ClonedSync);
        then!(env_state).eq(&SomeState::ClonedSync);
    }
}

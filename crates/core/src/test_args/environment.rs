use std::future::Future;
use std::ops::{Deref, DerefMut};

/// Infrastructure state shared by tests in a test tree.
///
/// Environments hold the context tests need to run — database pools, mocked external
/// services, shared clients — as opposed to **test state**, which is produced by business
/// actions and passed from parent to child via `Given`. Keeping the two separate makes it
/// clear what a test proves versus what infrastructure enables it.
///
/// Environments can evolve while going down the test tree: an environment is created from
/// its [`Base`](Environment::Base) environment, forming a chain that starts at `()`.
/// Creation is always async — even for sync tests — so it can perform async initialization
/// when needed.
///
/// ```
/// use testscribe_core::test_args::Environment;
///
/// struct InitialEnv {
///     value: i32,
/// }
///
/// impl Environment for InitialEnv {
///     /// This is a root environment, so it is created from `()`.
///     type Base = ();
///
///     async fn create(_base: Self::Base) -> Self {
///         InitialEnv { value: 54 }
///     }
/// }
///
/// /// An environment created by transforming `InitialEnv`.
/// struct NextEnv {
///     value: bool,
/// }
///
/// impl Environment for NextEnv {
///     type Base = InitialEnv;
///
///     async fn create(base: Self::Base) -> Self {
///         NextEnv { value: base.value > 0 }
///     }
/// }
/// ```
///
/// If `create` provisions external resources (files, databases, ...), it should clean up
/// leftovers from previous runs, so every execution starts from a known state.
#[diagnostic::on_unimplemented(
    message = "`{Self}` is not an `Environment`",
    label = "this type needs to implement `Environment`",
    note = "an `Env<{Self}>` argument requires `impl Environment for {Self}` (use `type Base = ();` for a root environment)"
)]
pub trait Environment {
    /// The environment this one is created from; use `()` for a root environment.
    type Base: Environment;
    /// Creates the environment from its base. Always async, even when used by sync tests.
    fn create(base: Self::Base) -> impl Future<Output = Self>;
}

impl Environment for () {
    type Base = ();
    async fn create(_base: Self::Base) -> Self {}
}

/// Test argument that provides mutable access to an [`Environment`].
///
/// Declaring an `Env<E>` argument makes the framework create `E` (and its whole
/// [`Base`](Environment::Base) chain) before the test runs. The environment is dropped as
/// soon as no remaining test in the tree uses it.
///
/// The inner value can be accessed by dereferencing (`env.value`, via [`Deref`]/[`DerefMut`])
/// or by destructuring in the argument position: `Env(env): Env<E>`, which binds the
/// environment (a `&mut E`) directly — handy for splitting borrows across its fields.
///
/// Write `Env<E>` the same way in sync and async tests — the `#[testscribe]` macro inserts the
/// anonymous lifetime (`Env<'_, E>`) that async functions require, so you never spell it out
/// (an explicit `Env<'_, E>` still works if you prefer it).
///
/// ```ignore
/// #[testscribe(standalone)]
/// #[test]
/// fn env_initialized(e: Env<InitialEnv>) {
///     then!(e.value => initial_env_value).eq(54);
/// }
///
/// #[testscribe(standalone)]
/// #[tokio::test]
/// async fn async_env_initialized(e: Env<InitialEnv>) {
///     then!(e.value => initial_env_value).eq(54);
/// }
/// ```
pub struct Env<'a, E>(pub &'a mut E)
where
    E: Environment;

impl<E> Deref for Env<'_, E>
where
    E: Environment,
{
    type Target = E;

    fn deref(&self) -> &Self::Target {
        self.0
    }
}

impl<E> DerefMut for Env<'_, E>
where
    E: Environment,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0
    }
}

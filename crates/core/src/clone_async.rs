//! Async counterpart of [`Clone`], used by `#[testscribe(cloneable_async)]`.

/// Async counterpart of [`Clone`] for test state and environments that can only be
/// duplicated in an async context.
///
/// Used by tests marked `#[testscribe(cloneable_async)]`: instead of re-executing the parent
/// test for every child branch, the framework clones its state (and environment) and hands
/// each sibling an independent copy.
///
/// When the state wraps an external resource (database, file, ...), `clone_async` must
/// produce a truly independent copy of that resource, not a second handle to the same one.
/// For example, a PostgreSQL environment can run
/// `CREATE DATABASE new_name WITH TEMPLATE current_name` and return a pool connected to the
/// fresh database.
///
/// For state that can be cloned synchronously, implement [`Clone`] and use
/// `#[testscribe(cloneable)]` instead.
///
/// If the type is plain [`Clone`] and only needs `CloneAsync` to satisfy
/// `#[testscribe(cloneable_async)]` (e.g. state living alongside an async-cloned
/// environment), derive it: `#[derive(Clone, CloneAsync)]`. The derive ships with the
/// `testscribe` crate and simply delegates to `Clone`.
#[diagnostic::on_unimplemented(
    message = "`{Self}` does not implement `CloneAsync`, required by `#[testscribe(cloneable_async)]`",
    note = "for a plain `Clone` type, derive it alongside `Clone`: `#[derive(Clone, CloneAsync)]`",
    note = "implement it by hand only when cloning needs async work (e.g. snapshotting a database)"
)]
pub trait CloneAsync {
    fn clone_async(&self) -> impl Future<Output = Self>;
}

impl CloneAsync for () {
    async fn clone_async(&self) -> Self {}
}

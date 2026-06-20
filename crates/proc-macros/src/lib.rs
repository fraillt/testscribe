//! Procedural macros for the `testscribe` test framework.
//!
//! This crate shouldn't be used directly, both macros are re-exported by the `testscribe` crate,
//! which is the main entry point for users.

mod config;
mod derive_clone_async;
mod derive_param_display;
mod expand_params;
mod expand_test;
mod test_fn;
mod utils;

use config::Config;
use expand_params::expand_params;
use expand_test::expand_test;
use proc_macro::TokenStream;
use syn::{DeriveInput, parse_macro_input};
use test_fn::TestFn;

use crate::derive_clone_async::expand_clone_async;
use crate::derive_param_display::expand_param_display;

/// Turns a function into a node of a `testscribe` test tree.
///
/// Every annotated function becomes a test node:
/// - the function is registered in the global test tree (`testscribe::CASES`),
/// - a marker type named after the function, converted to PascalCase, is generated
///   (e.g. `one_and_two_is_added` generates `OneAndTwoIsAdded`); child tests use it to declare
///   their parent via the [`Given`] argument,
/// - the function's return value becomes the node's state, which child tests build upon,
/// - a local [`then!`](#the-then-macro) macro becomes available inside the function body for assertions.
///
/// ```ignore
/// use testscribe::{report::basic::CheckEq, test_args::Given, testscribe};
///
/// /// A root test: `standalone` registers it with the standard test runner,
/// /// so `cargo test` runs it together with its whole subtree.
/// #[testscribe(standalone)]
/// fn one_and_two_is_added() -> i32 {
///     let sum = 1 + 2;
///     then!(sum).eq(3);
///     sum
/// }
///
/// /// A child test: receives the parent's returned state via `Given`.
/// #[testscribe]
/// fn another_6_is_added(state: Given<OneAndTwoIsAdded>) {
///     then!(*state + 6 => sum).eq(9);
/// }
/// ```
///
/// Running `one_and_two_is_added` produces output like:
/// ```text
/// | 7.360μs|Given one and two is added
/// |       -|  Then sum is equal to 3
/// | 1.860μs|  When another 6 is added
/// |       -|    Then sum is equal to 9
/// ```
///
/// # Test function arguments
///
/// A test function can declare up to three arguments, in any order, each at most once.
/// Every argument must be wrapped in one of these types from `testscribe::test_args`:
///
/// - [`Given<Parent>`] — the state returned by the parent test. `Parent` is the marker type
///   generated for the parent test function. Declaring it makes this test a child of `Parent`.
/// - [`Env<E>`] — an environment implementing the [`Environment`] trait. Write `Env<E>` in both
///   sync and async tests; the macro inserts the anonymous lifetime async functions require.
/// - [`Param<P>`] — a parameter provided by a `#[testscribe(params)]` function (see below).
///   The test and its subtree run once for each parameter.
///
/// Async functions are supported; note that if a parent test is async, all of its child tests
/// must be async as well.
///
/// # Options
///
/// `#[testscribe]` accepts a comma-separated list of options: `params`, `standalone`,
/// `cloneable`, `cloneable_async` and `tags`.
///
/// ## `standalone`
///
/// Generates a wrapper function so the standard test runner (libtest) can execute this node
/// together with its whole subtree. Only root tests (tests without a `Given` argument) can
/// be standalone.
///
/// The wrapper — not your function — is what the test harness invokes, so the harness
/// attribute is passed *inside* `standalone(...)`. A bare `standalone` on a sync test
/// defaults to `#[test]`; async tests must name their harness:
///
/// ```ignore
/// /// Sync root: the wrapper gets `#[test]` automatically.
/// #[testscribe(standalone)]
/// fn sync_root() {}
///
/// /// Async root: name the harness (any test attribute path works).
/// #[testscribe(standalone(tokio::test))]
/// async fn async_root() {}
///
/// /// Harness arguments and extra wrapper attributes are supported.
/// #[testscribe(standalone(tokio::test(flavor = "multi_thread"), ignore))]
/// async fn multi_threaded_root() {}
/// ```
///
/// Attributes written on the function itself stay on the test body, so `#[allow(...)]` and
/// friends work as expected. `#[cfg(...)]` is special: it is propagated to everything the
/// macro generates (wrapper, tree registration, marker type), so a feature-gated test
/// disappears entirely. Writing a harness attribute such as `#[test]` or `#[tokio::test]`
/// on the function is a compile error pointing to `standalone(...)`.
///
/// Requires the `standalone` feature of the `testscribe` crate (enabled by default).
/// Alternatively, skip `standalone` entirely and run tests with a custom runner
/// (see `testscribe::standalone::run_all_sync` and the `harness = false` Cargo option).
///
/// ## `cloneable` / `cloneable_async`
///
/// By default, when a test has multiple children, the parent re-executes for every child branch,
/// so each branch starts from fresh state. `cloneable` avoids that re-execution: the framework
/// clones the returned state (and the environment) with [`Clone`] and hands each sibling its own
/// copy. `cloneable_async` does the same using the `CloneAsync` trait for state that can only be
/// cloned in an async context (e.g. creating a database from a template); it requires the test
/// function itself to be async. Only one of the two can be used.
///
/// ## `tags=[...]`
///
/// Attaches tags to the test node, e.g. `#[testscribe(tags=[slow, db])]`. Tags are interpreted
/// by the runner. The standalone runner skips tests tagged `ignore` by default (include them
/// with `--include-ignored`), and supports filtering by tag with bracket syntax:
/// `cargo test -- [slow]` runs only tests tagged `slow`.
///
/// ## `params`
///
/// Switches the macro into parameter-provider mode; the function is not a test. It must return
/// a `Vec<T>` where `T` implements `ParamDisplay` (usually via `#[derive(ParamDisplay)]`).
/// A marker type (PascalCase function name) implementing `Parameter` is generated, which tests
/// reference through the `Param` argument:
///
/// ```ignore
/// use testscribe::{ParamDisplay, report::basic::CheckEq, test_args::Param, testscribe};
///
/// #[derive(Clone, ParamDisplay)]
/// struct TestCaseParam {
///     nr: i32,
/// }
///
/// #[testscribe(params)]
/// fn test_params() -> Vec<TestCaseParam> {
///     vec![TestCaseParam { nr: 1 }, TestCaseParam { nr: 2 }]
/// }
///
/// /// Runs once for each parameter; child tests run for each parameter as well.
/// #[testscribe(standalone)]
/// fn add_numbers(p: Param<TestParams>) -> i32 {
///     then!(p.nr => nr).eq(p.nr);
///     p.nr
/// }
/// ```
///
/// `params` cannot be combined with any other option.
///
/// # The `then!` macro
///
/// Inside the test function body, a local `then!` macro is generated with three forms:
///
/// | Form | Returns | Use case |
/// |------|---------|----------|
/// | `then!(variable)` | `VerifyValue` | verify one concrete value; output uses the variable name |
/// | `then!(expression => alias)` | `VerifyValue` | verify an expression; output uses the alias |
/// | `then!("statement")` | `VerifyStatement` | run a closure under a narrative statement |
///
/// The returned objects carry no methods of their own; checks come from extension traits in
/// `testscribe::report::basic`, which must be imported:
///
/// - `CheckEq` — `then!(sum).eq(3)`, `then!(sum).ne(5)`
/// - `CheckRun` — `then!("payment is accepted").run(|| ...)`; the check fails if the closure
///   panics, returns `false`, or returns an `Err`
/// - `CheckAsyncRun` — `then!("...").run_async(async || ...).await` (async tests only)
/// - `CheckParams` — `then!("...").params(list).run(|item| ...)` for table-style checks
///
/// Custom, domain-specific checks can be added by implementing a trait on
/// `VerifyValue` — see `testscribe::report::VerifyValueExposed`.
///
/// # Compile errors
///
/// - test arguments not wrapped in `Given`, `Env` or `Param`, or wrapped more than once
/// - `standalone` on a test with a `Given` argument (standalone tests must be roots)
/// - bare `standalone` on an async test (the harness must be named: `standalone(tokio::test)`)
/// - a harness attribute (`#[test]`, `#[tokio::test]`, ...) on a standalone test function
///   instead of inside `standalone(...)`
/// - `cloneable_async` on a sync test function
/// - `cloneable` combined with `cloneable_async`
/// - `params` combined with any other option
/// - `then!(expression)` without an alias (`=> alias` is required for expressions)
///
/// [`Given`]: https://docs.rs/testscribe/latest/testscribe/test_args/struct.Given.html
/// [`Given<Parent>`]: https://docs.rs/testscribe/latest/testscribe/test_args/struct.Given.html
/// [`Env<E>`]: https://docs.rs/testscribe/latest/testscribe/test_args/struct.Env.html
/// [`Param<P>`]: https://docs.rs/testscribe/latest/testscribe/test_args/struct.Param.html
/// [`Environment`]: https://docs.rs/testscribe/latest/testscribe/test_args/trait.Environment.html
#[proc_macro_attribute]
pub fn testscribe(attr: TokenStream, item: TokenStream) -> TokenStream {
    let config = parse_macro_input!(attr as Config);
    let test_fn = parse_macro_input!(item as TestFn);
    match config {
        Config::Test(test_config) => TokenStream::from(expand_test(test_config, test_fn)),
        Config::Params => TokenStream::from(expand_params(test_fn)),
    }
}

/// Derives `testscribe::test_args::ParamDisplay`, which controls how parameter values are
/// rendered as table rows in test output.
///
/// Only structs with named fields are supported. Field names become the column headers, and
/// each field value is rendered with its [`Display`](std::fmt::Display) implementation by default.
///
/// Rendering of individual fields can be customized with the `#[pd]` attribute:
/// - `#[pd(debug)]` — render the field with [`Debug`](std::fmt::Debug) (`{:?}`) instead of `Display`
/// - `#[pd(custom = path::to_fn)]` — render with a custom function `fn(&FieldType) -> String`
/// - `#[pd(hide)]` — omit the field from the displaying entirely (no column header, no value).
///
/// ```ignore
/// use testscribe::ParamDisplay;
///
/// fn cents_to_eur(cents: &u32) -> String {
///     format!("{}.{:02}€", cents / 100, cents % 100)
/// }
///
/// #[derive(Clone, ParamDisplay)]
/// struct Payment {
///     order_id: u32,
///     #[pd(custom = cents_to_eur)]
///     amount: u32,
///     #[pd(debug)]
///     accepted: Option<bool>,
///     // carried into the test but not shown in the table
///     #[pd(hide)]
///     gateway_response: GatewayResponse,
/// }
/// ```
///
/// Used together with `#[testscribe(params)]` parameter providers and with
/// `then!("...").params(list)` table-style checks; see the [`testscribe`](macro@testscribe)
/// attribute macro for details.
#[proc_macro_derive(ParamDisplay, attributes(pd))]
pub fn param_display(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    expand_param_display(input)
}

/// Derives `testscribe::clone_async::CloneAsync` by delegating to the type's [`Clone`]
/// implementation.
///
/// `Clone` is required and is **not** derived automatically — spell out both, the same way
/// `#[derive(Copy, Clone)]` works:
///
/// ```ignore
/// use testscribe::CloneAsync;
///
/// #[derive(Debug, Clone, CloneAsync)]
/// struct OrderState {
///     customer: Uuid,
///     order: Uuid,
/// }
/// ```
///
/// Works on any type shape (structs, enums, tuple structs), since the generated
/// implementation is simply the async version of whatever your `Clone` does:
/// `async fn clone_async(&self) -> Self { self.clone() }`.
///
/// Use it for plain-`Clone` state returned by tests marked `#[testscribe(cloneable_async)]`,
/// which requires `CloneAsync` on both the test state and the environment.
///
/// When `Clone` and `CloneAsync` must genuinely differ — e.g. `Clone` copies a cheap handle
/// while `CloneAsync` duplicates an external resource such as a database — implement
/// `CloneAsync` manually instead. This distinction is also why no blanket
/// `impl<T: Clone> CloneAsync for T` exists: it would make such manual implementations
/// impossible.
#[proc_macro_derive(CloneAsync)]
pub fn clone_async(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    expand_clone_async(input)
}

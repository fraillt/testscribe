//! Test framework for building **test trees** — tests that build on each other's state and
//! produce output that reads like a domain narrative.
//!
//! Instead of writing isolated tests that each rebuild their own setup, every `testscribe`
//! test performs a single business action, asserts on its side effects, and returns its
//! state so that child tests can continue from there. Running a tree prints a readable story:
//!
//! ```text
//! | 7.360μs|Given one and two is added
//! |       -|  Then sum is equal to 3
//! | 1.860μs|  When another 6 is added
//! |       -|    Then sum is equal to 9
//! ```
//!
//! # Quick start
//!
//! Annotate test functions with [`#[testscribe]`](macro@testscribe). A root test is marked
//! `standalone` (so `cargo test` runs it together with its whole subtree) and each child
//! test declares its parent with the [`Given`](test_args::Given) argument:
//!
//! ```no_run
//! use testscribe::prelude::*;
//!
//! #[testscribe(standalone)]
//! fn one_and_two_is_added() -> i32 {
//!     let sum = 1 + 2;
//!     then!(sum).eq(3);
//!     sum
//! }
//!
//! #[testscribe]
//! fn another_6_is_added(state: Given<OneAndTwoIsAdded>) {
//!     then!(*state + 6 => sum).eq(9);
//! }
//! # fn main() {}
//! ```
//!
//! Key pieces:
//! - [`#[testscribe]`](macro@testscribe) — defines test nodes, parameter providers and the
//!   `then!` assertion macro; see its documentation for all options
//! - [`test_args`] — test function arguments: [`Given`](test_args::Given) (parent state),
//!   [`Env`](test_args::Env) (infrastructure environment), [`Param`](test_args::Param)
//!   (parameterized tests)
//! - [`report`] — built-in checks (`eq`, `ne`, `contains`, `run`, `run_async`, `params`) and
//!   support for custom domain-specific checks
//! - [`ParamDisplay`] derive — renders parameters as table rows in test output
//! - [`clone_async`] — cloning test state/environments to avoid re-running parent tests
//!
//! # Cargo features
//!
//! - `standalone` *(enabled by default)* — integrates test trees with the standard test
//!   runner via `#[testscribe(standalone)]`, and exposes the [`standalone`] module with
//!   utilities for building custom runners (e.g. `standalone::run_all_sync` together with
//!   `harness = false`).
//! - `detached` — experimental remote-controlled test runner, exposed as the `detached`
//!   module.
//!
//! # Learn more
//!
//! - [Foundations](https://github.com/fraillt/testscribe/blob/main/crates/testscribe/docs/foundations.md) —
//!   the test tree philosophy and the SHAPE loop for growing a tree
//! - [Features showcase](https://github.com/fraillt/testscribe/tree/main/crates/testscribe/tests) —
//!   compact syntax examples for every feature
//! - [Advanced techniques](https://github.com/fraillt/testscribe/blob/main/crates/testscribe/docs/advanced_techniques.md) —
//!   environments, custom checks, state cloning, custom runners
//!
//! All of these guides are also shipped inside the published crate (`docs/` folder), so they
//! are available offline wherever the crate source is — including your local cargo registry.

// re-export macros
pub use testscribe_proc_macros::CloneAsync;
pub use testscribe_proc_macros::ParamDisplay;
pub use testscribe_proc_macros::testscribe;

pub use testscribe_core::*;

// Co-locate the `CloneAsync` *trait* at the crate root next to the `CloneAsync` *derive* above.
// They are different items in different namespaces (type vs. macro), so a single
// `use testscribe::CloneAsync;` brings in both — the same trick `std` uses for `Debug`
// (`core::fmt`'s `pub use macros::Debug;`) and `serde` uses for `Serialize`.
pub use testscribe_core::clone_async::CloneAsync;

/// Everything needed to write tests, in one glob import: `use testscribe::prelude::*;`.
///
/// Pulls in the [`#[testscribe]`](macro@testscribe) attribute, the test-function arguments
/// ([`Given`](test_args::Given), [`Env`](test_args::Env), [`Param`](test_args::Param) and the
/// [`Environment`](test_args::Environment) trait), every built-in check so the right extension
/// trait is always in scope ([`eq`/`ne`](report::basic::CheckEq),
/// [`contains`](report::basic::CheckContains), [`run`](report::basic::CheckRun),
/// [`run_async`](report::basic::CheckAsyncRun), [`params`](report::basic::CheckParams)), the
/// [`CloneAsync`] and [`ParamDisplay`] derives, and the full toolkit for writing custom checks
/// ([`VerifyValue`](report::VerifyValue) / [`VerifyValueExposed`](report::VerifyValueExposed) and
/// [`VerifyStatement`](report::VerifyStatement) / [`VerifyStatementExposed`](report::VerifyStatementExposed),
/// reported via [`VerifyOutcome`](report::VerifyOutcome)).
///
/// The `then!` macro is *not* here: it is injected into every `#[testscribe]` function body, so
/// it is never imported.
pub mod prelude {
    // Attribute macro that defines test nodes, plus the two derives.
    pub use crate::{CloneAsync, ParamDisplay, testscribe};

    // Test-function arguments.
    pub use crate::test_args::{Env, Environment, Given, Param};

    // Built-in checks behind the `then!` macro — one extension trait per check form.
    pub use crate::report::basic::{CheckAsyncRun, CheckContains, CheckEq, CheckParams, CheckRun};

    // Building blocks for writing project-local custom checks, for both the value form
    // (`then!(x)`) and the statement form (`then!("...")`).
    pub use crate::report::{
        VerifyOutcome, VerifyStatement, VerifyStatementExposed, VerifyValue, VerifyValueExposed,
    };
}

/// Test runner used by `#[testscribe(standalone)]` tests, plus utilities for custom runners.
#[cfg(feature = "standalone")]
pub mod standalone {
    pub use testscribe_standalone::*;
}

/// Experimental remote-controlled test runner.
#[cfg(feature = "detached")]
pub mod detached {
    pub use testscribe_detached::*;
}

#[doc(hidden)]
pub use linkme;

/// All test cases registered by the [`#[testscribe]`](macro@testscribe) macro, collected at
/// link time.
///
/// Only needed when building a custom test runner (with `harness = false` in Cargo.toml),
/// e.g. `standalone::run_all_sync(&CASES, Arguments::from_args())`.
#[linkme::distributed_slice]
pub static CASES: [testscribe_core::test_case::TestCase];

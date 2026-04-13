//! Argument types for test functions.
//!
//! Arguments of a `#[testscribe]` test function must be wrapped in one of these types:
//! - [`Given`] — state returned by the parent test,
//! - [`Env`] — an [`Environment`] holding infrastructure state,
//! - [`Param`] — one parameter from a `#[testscribe(params)]` provider.

mod environment;
mod params;
mod parent;

pub use environment::{Env, Environment};
pub use params::{Param, ParamDisplay, Parameter};
pub use parent::{Given, ParentTest};

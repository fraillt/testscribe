//! An **experimental** remote-controlled test runner for the
//! [testscribe](https://docs.rs/testscribe) test framework.
//!
//! Instead of running a test tree under the local `cargo test` harness, this runner lets the tree
//! be driven over a connection by an external driver. This enables scenarios such as inspecting or
//! stepping through a tree from a separate process.
//!
//! # Stability
//!
//! ⚠️ This crate is experimental and unstable; its API may change or be removed in any release.
//! Most users want the default [`testscribe-standalone`](https://docs.rs/testscribe-standalone)
//! runner instead. Enable this one through the main crate's `detached` feature.
//!
//! The [`runtime`] module hosts the test tree (backend) side and [`driver`] hosts the controlling
//! (frontend) side.

#[doc(hidden)]
pub mod driver;
#[doc(hidden)]
pub mod runtime;

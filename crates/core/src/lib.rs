//! Core library of testscribe, containing the main logic of the test runner and the test tree structure.
//! This crate shouldn't be used directly, but is re-exported by `testscribe` crate, which is the main entry point for users.

pub mod clone_async;
pub mod report;
pub mod test_args;

// These modules are not used in regular tests; they are the building blocks
// for custom test runners (see also the `testscribe-standalone` crate).
#[doc(hidden)]
pub mod processor;
#[doc(hidden)]
pub mod test_case;
#[doc(hidden)]
pub mod tests_tree;

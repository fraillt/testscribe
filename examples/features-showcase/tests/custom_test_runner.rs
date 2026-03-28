// For demonstration purposes some test function doesn't have assertions, so we suppress the unused macro warning.
#![allow(unused_macros)]

use std::process::ExitCode;
use testscribe::standalone::args::Arguments;
use testscribe::standalone::run_all_sync;
use testscribe::test_args::Given;
use testscribe::{CASES, testscribe};

/// This is a root test (it has no parent), but it does not use the `standalone` attribute
/// because the standard test runner is disabled and all tests run via the custom runner in `main`.
#[testscribe()]
fn first_test_initialized() {}

/// Custom runners can support tags. In `run_all_sync` from the standalone crate,
/// tests with the `ignore` tag are skipped by default, but you can include them with
/// the `--include-ignored` flag.
#[testscribe(tags=[ignore])]
fn ignore_tag_is_added(_: Given<FirstTestInitialized>) {}

#[testscribe]
fn child_will_be_ignored_as_well(_: Given<IgnoreTagIsAdded>) {}

/// `run_all_sync` also supports filtering by any tag.
/// For example, running `cargo test --package features-showcase --test custom_test_runner -- [slow]`
/// will run all tests with slow tag,
/// `[]` brackets are required to filter by tags, otherwise it will be treated as a substring filter for test names.
#[testscribe(tags=[slow])]
fn long_running_test() {}

/// Disable the default test runner in Cargo.toml (`harness = false`) and use a custom runner for all tests.
/// Running this test will give this outcome:
/// ```text
/// | 6.780μs|Given first test initialized
///?| 0.000ns|  When ignore tag is added
///?| 0.000ns|    When child will be ignored as well
/// | 0.011ms|Given long running test
/// ```
///
fn main() -> ExitCode {
    // Run all synchronous tests using a helper from the `standalone` crate, which prints test results.
    // It also parses command line arguments, similar to libtest.
    // For example `cargo test --package features-showcase --test custom_test_runner -- --include-ignored`
    // runs all tests, including those with the `ignore` tag.
    run_all_sync(&CASES, Arguments::from_args())
        .unwrap()
        .exit_code()
}

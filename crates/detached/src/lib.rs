//! An **experimental** connection from [testscribe](https://docs.rs/testscribe) tests to a
//! remote frontend that shows the run, and may choose what runs.
//!
//! The tests still run under `cargo test`. When the environment variable
//! [`FRONTEND_ENV_NAME`] names a frontend, the test binary connects to it, sends its test
//! trees, and every standalone tree asks the frontend what to run before it starts and reports
//! each test as it goes. The terminal output stays what it is.
//!
//! ```text
//! TESTSCRIBE_FRONTEND=127.0.0.1:7357 cargo test
//! ```
//!
//! Without the variable, or when nothing answers at that address, the tests run as usual.
//!
//! [`BATCH_ENV_NAME`] names the batch the results belong to. A frontend stores a named batch,
//! can compare two of them, and remembers per batch what to run the next time:
//!
//! ```text
//! TESTSCRIBE_FRONTEND=127.0.0.1:7357 TESTSCRIBE_BATCH_ID=before-refactoring cargo test
//! ```
//!
//! # Stability
//!
//! ⚠️ This crate is experimental and unstable; its API and its [`protocol`] may change in any
//! release. Enable it through the `detached` feature of the main crate.

mod connection;
pub mod protocol;

use std::time::Duration;

use futures::channel::oneshot;
use futures::executor::block_on;
use testscribe_core::processor::logger::TestStatusUpdate;
use testscribe_core::test_case::{FqFnName, TestCase};
use testscribe_core::tests_tree::TestsTree;
use testscribe_standalone::{Selection, TreeObserver, run_observed};

use crate::connection::{Frontend, Outgoing};
use crate::protocol::ToFrontend;

/// Names the frontend to connect to: `host:port`, or a whole `ws://` url.
pub const FRONTEND_ENV_NAME: &str = "TESTSCRIBE_FRONTEND";

/// Names the batch the results of this run belong to: any text. Without it, the frontend
/// keeps the results in its batch `default`, which it does not store.
pub const BATCH_ENV_NAME: &str = "TESTSCRIBE_BATCH_ID";

/// Used by the `#[testscribe]` macro to run a sync `standalone` test.
pub fn run_sync(
    test_cases: &'static [TestCase],
    module_path: &'static str,
    test_name: &'static str,
) -> Result<(), String> {
    block_on(run_async(test_cases, module_path, test_name))
}

/// Used by the `#[testscribe]` macro to run an async `standalone` test.
pub async fn run_async(
    test_cases: &'static [TestCase],
    module_path: &'static str,
    test_name: &'static str,
) -> Result<(), String> {
    match connection::frontend(test_cases) {
        Some(frontend) => {
            let mut observer = FrontendObserver {
                frontend,
                root: FqFnName::new(module_path, test_name).to_string(),
            };
            run_observed(test_cases, module_path, test_name, &mut observer).await
        }
        None => testscribe_standalone::run_async(test_cases, module_path, test_name).await,
    }
}

/// Reports one standalone tree to the frontend.
struct FrontendObserver {
    frontend: &'static Frontend,
    root: String,
}

impl TreeObserver for FrontendObserver {
    async fn select(&mut self, _tree: &TestsTree) -> Selection {
        let (reply, selection) = oneshot::channel();
        self.frontend.send(Outgoing::Ready {
            root: self.root.clone(),
            reply,
        });
        // The connection answers for a frontend that does not: with everything.
        selection.await.unwrap_or(Selection::All)
    }

    fn observe(&mut self, test: &'static TestCase, update: &TestStatusUpdate, elapsed: Duration) {
        self.frontend.send(Outgoing::Message(
            ToFrontend::Test {
                root: &self.root,
                test: test.name,
                update,
                elapsed_us: elapsed.as_micros() as u64,
            }
            .to_json(),
        ));
    }

    async fn finished(&mut self, success: bool) {
        self.frontend.send(Outgoing::Message(
            ToFrontend::TreeFinished {
                root: &self.root,
                success,
            }
            .to_json(),
        ));
        // The process may end right after its last test: nothing may still be on its way.
        let (done, flushed) = oneshot::channel();
        self.frontend.send(Outgoing::Flush(done));
        let _ = flushed.await;
    }
}

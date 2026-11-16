//! What tests and frontend say to each other: JSON text messages over a WebSocket, which the
//! test binary opens at `/tests`.
//!
//! The test binary starts with [`ToFrontend::Hello`], which carries every test tree it has.
//! Then, for each standalone tree the harness starts:
//!
//! 1. [`ToFrontend::TreeReady`], answered by [`FromFrontend::Run`] with what to run;
//! 2. a [`ToFrontend::Test`] for every status update of every test;
//! 3. [`ToFrontend::TreeFinished`].
//!
//! The session is over when the connection closes: the test binary has exited.

use serde::{Deserialize, Serialize};
use testscribe_core::processor::logger::TestStatusUpdate;
use testscribe_core::test_case::FqFnName;
use testscribe_core::tests_tree::TestsTree;

pub const PROTOCOL_VERSION: u32 = 1;

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ToFrontend<'a> {
    Hello {
        protocol: u32,
        /// The file name of the test binary, e.g. `checkout_flow-31acdf8590b2be92`.
        binary: String,
        /// The package the tests belong to, when run by cargo. The `filename` of a test or a
        /// check is relative to the workspace this package is in.
        manifest_dir: Option<String>,
        cwd: Option<String>,
        pid: u32,
        /// Every tree of the binary, also the ones the harness will not start.
        trees: &'a [TestsTree],
    },
    /// The harness started this tree; it waits to be told what to run.
    TreeReady {
        root: &'a str,
    },
    Test {
        root: &'a str,
        test: FqFnName<'static>,
        update: &'a TestStatusUpdate,
        /// Since the tree started, in microseconds.
        elapsed_us: u64,
    },
    TreeFinished {
        root: &'a str,
        success: bool,
    },
}

impl ToFrontend<'_> {
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("a message to the frontend is plain data")
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FromFrontend {
    /// What to run of the tree that is ready. A root is named `module::path::TestName`.
    Run { root: String, select: Select },
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Select {
    All,
    /// The tests on these paths, for every path the names from the root down.
    Paths {
        paths: Vec<Vec<String>>,
    },
    None,
}

use std::borrow::Cow;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use testscribe_core::processor::logger::TestStatusUpdate;
use testscribe_core::test_case::FqFnName;
use testscribe_standalone::panic_hook::PanicDetails;

#[derive(Debug, Deserialize)]
pub struct FqFnNameOwned {
    pub path: Cow<'static, str>,
    pub name: Cow<'static, str>,
}

impl FqFnNameOwned {
    pub fn as_fq_fn_name(&self) -> FqFnName<'_> {
        FqFnName::new(&self.path, &self.name)
    }
}

impl From<FqFnName<'static>> for FqFnNameOwned {
    fn from(value: FqFnName<'static>) -> Self {
        Self {
            path: Cow::Borrowed(value.path),
            name: Cow::Borrowed(value.name),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind")]
pub enum TestTreeFilter {
    RunAll,
    RunPaths { paths: Vec<Vec<FqFnNameOwned>> },
}

#[derive(Debug, Deserialize)]
pub struct RunTestTree {
    pub id: u64,
    pub root_test: FqFnNameOwned,
    pub filter: TestTreeFilter,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind")]
pub enum CommandMsg {
    RunTestTrees { trees: Vec<RunTestTree> },
    EnablePanicsCollector,
    DisablePanicsCollector,
}

#[derive(Debug, Serialize)]
pub enum TestTreeStatusUpdate {
    Started,
    Finished,
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind")]
pub enum StatusMsg {
    TestTreeStatus {
        tree_id: u64,
        update: TestTreeStatusUpdate,
    },
    TestStatus {
        tree_id: u64,
        update: TestStatusUpdate,
        elapsed: Duration,
    },
    Panic {
        details: PanicDetails,
    },
    InvalidCommandError {
        message: String,
    },
}

use std::time::Duration;

use serde::Serialize;

use crate::test_case::{FqFnName, TestCase};

#[derive(Debug, Clone, Serialize)]
pub struct ParamInfo {
    pub headers: Vec<&'static str>,
    pub display_str: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TestRunInfo {
    pub depth: usize,
    pub run_count: usize,
    pub param_info: Option<ParamInfo>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind")]
pub enum VerifyOutcome {
    Success,
    Failure { details: String },
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind")]
pub enum TestUpdate {
    Verified {
        message: String,
        line_nr: u32,
        file: &'static str,
        outcome: VerifyOutcome,
    },
    ParamsStarted {
        message: String,
        line_nr: u32,
        file: &'static str,
        header: Vec<&'static str>,
    },
    ParamVerified {
        index: usize,
        row_fields: Vec<String>,
        outcome: VerifyOutcome,
    },
    ParamsFinished,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(tag = "kind")]
pub enum PanicLocation {
    Test,
    Environment,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind")]
pub enum SkipReason {
    Panicked {
        name: FqFnName<'static>,
        location: PanicLocation,
        message: String,
    },
    Ignored {
        name: FqFnName<'static>,
    },
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind")]
pub enum TestStatusUpdate {
    Started {
        info: TestRunInfo,
    },
    Updated {
        info: TestUpdate,
    },
    Finished {
        panic_message: Option<String>,
    },
    Skipped {
        info: TestRunInfo,
        reason: SkipReason,
    },
}

pub trait Logger {
    fn log(&mut self, test: &'static TestCase, update: TestStatusUpdate, elapsed: Duration);
}

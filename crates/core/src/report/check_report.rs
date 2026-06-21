use std::error::Error;

use serde::Serialize;

use crate::processor::logger::TestUpdate;
use crate::report::TestReport;

/// Outcome of a single check, reported through [`CheckReporter::set_outcome`].
///
/// This is the core check interface: every check — built-in or custom — ultimately produces a
/// `VerifyOutcome` and hands it to a [`CheckReporter`]. The `From` implementations below let
/// `run`/`run_async` closures return a `()`, `bool` or `Result` and have it mapped to an
/// outcome automatically.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind")]
pub enum VerifyOutcome {
    Success,
    /// Failed check; `details` (e.g. the actual value) are shown in the test output.
    Failure {
        details: String,
    },
}

/// `()` always converts to `Success`; closures that only assert/panic rely on this.
impl From<()> for VerifyOutcome {
    fn from(_value: ()) -> Self {
        VerifyOutcome::Success
    }
}

/// `true` converts to `Success`, `false` to `Failure`.
impl From<bool> for VerifyOutcome {
    fn from(value: bool) -> Self {
        if value {
            VerifyOutcome::Success
        } else {
            VerifyOutcome::Failure {
                details: "result is false".to_string(),
            }
        }
    }
}

/// `Ok` converts to `Success`, `Err` to `Failure` with the error message as details.
impl<T, R> From<Result<T, R>> for VerifyOutcome
where
    R: Error,
{
    fn from(value: Result<T, R>) -> Self {
        match value {
            Ok(_) => VerifyOutcome::Success,
            Err(err) => VerifyOutcome::Failure {
                details: format!("error: {err}"),
            },
        }
    }
}

/// Reports the outcome of a single check to the test output.
///
/// Accessed through [`VerifyValueExposed`](crate::report::VerifyValueExposed) /
/// [`VerifyStatementExposed`](crate::report::VerifyStatementExposed) when implementing
/// custom checks; call [`set_outcome`](CheckReporter::set_outcome) exactly once.
pub struct CheckReporter<'a> {
    line: u32,
    file: &'static str,
    report: &'a mut TestReport,
}

impl<'a> CheckReporter<'a> {
    pub fn new(line: u32, file: &'static str, report: &'a mut TestReport) -> Self {
        Self { line, file, report }
    }
    /// Reports the check result; `message` is shown as the `Then ...` line in test output.
    pub fn set_outcome(self, message: String, outcome: VerifyOutcome) {
        self.report.update(TestUpdate::Verified {
            message,
            file: self.file,
            line_nr: self.line,
            outcome,
        });
    }

    pub fn into_param_check_reporter(
        self,
        message: String,
        header: Vec<&'static str>,
    ) -> ParamCheckReporter<'a> {
        self.report.update(TestUpdate::ParamsStarted {
            message,
            line_nr: self.line,
            file: self.file,
            columns_count: header.len(),
            header: Some(header),
        });
        ParamCheckReporter {
            report: self.report,
        }
    }

    pub fn into_param_check_reporter_no_header(
        self,
        message: String,
        columns_count: usize,
    ) -> ParamCheckReporter<'a> {
        self.report.update(TestUpdate::ParamsStarted {
            message,
            line_nr: self.line,
            file: self.file,
            columns_count,
            header: None,
        });
        ParamCheckReporter {
            report: self.report,
        }
    }
}

/// Reports the per-row outcomes of a table-style (params) check.
///
/// Created with [`CheckReporter::into_param_check_reporter`]; the table is finished
/// automatically when the reporter is dropped.
pub struct ParamCheckReporter<'a> {
    report: &'a mut TestReport,
}

impl ParamCheckReporter<'_> {
    /// Reports the outcome of one table row.
    pub fn set_param_outcome(&mut self, index: usize, row: Vec<String>, outcome: VerifyOutcome) {
        self.report.update(TestUpdate::ParamVerified {
            index,
            row,
            outcome,
        });
    }
}

impl Drop for ParamCheckReporter<'_> {
    fn drop(&mut self) {
        self.report.update(TestUpdate::ParamsFinished);
    }
}

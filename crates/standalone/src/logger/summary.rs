use std::process::ExitCode;

use std::{mem::take, time::Duration};

use testscribe_core::{
    processor::logger::{Logger, SkipReason, TestStatusUpdate, TestUpdate, VerifyOutcome},
    test_case::{FqFnName, TestCase},
};

use crate::panic_hook::PanicDetails;

#[derive(Debug, Clone)]
pub struct Failure {
    pub param_index: Option<usize>,
    pub message: String,
    pub line_nr: u32,
    pub file: &'static str,
    pub details: String,
}

struct ParamInfo {
    message: String,
    line_nr: u32,
    file: &'static str,
}

#[derive(Debug, Clone, Default)]
pub struct ExecutionSummary {
    pub passed: Vec<FqFnName<'static>>,
    pub failed: Vec<(FqFnName<'static>, Vec<Failure>)>,
    pub skipped: Vec<(FqFnName<'static>, SkipReason)>,
    pub panics: Vec<PanicDetails>,
}

impl ExecutionSummary {
    pub fn is_success(&self) -> bool {
        self.failed.is_empty()
    }

    pub fn extend(&mut self, other: &ExecutionSummary) {
        self.passed.extend(other.passed.clone());
        self.failed.extend(other.failed.clone());
        self.skipped.extend(other.skipped.clone());
        self.panics.extend(other.panics.clone());
    }

    /// Exits the application with an appropriate error code (0 if all tests
    /// have passed, 101 if there have been failures).
    pub fn exit_code(&self) -> ExitCode {
        if self.is_success() {
            ExitCode::SUCCESS
        } else {
            ExitCode::from(101)
        }
    }
}

pub struct TestsTreeLogger<'a, P: Logger> {
    printer: &'a mut P,
    summary: ExecutionSummary,
    is_first_run: bool,
    current_failures: Vec<Failure>,
    current_param: Option<ParamInfo>,
}

impl<'a, P: Logger> TestsTreeLogger<'a, P> {
    pub fn new(printer: &'a mut P) -> Self {
        Self {
            printer,
            summary: ExecutionSummary {
                passed: Vec::new(),
                failed: Vec::new(),
                skipped: Vec::new(),
                panics: Vec::new(),
            },
            is_first_run: false,
            current_failures: Vec::new(),
            current_param: None,
        }
    }
    pub fn into_summary(self) -> ExecutionSummary {
        self.summary
    }
}

impl<P: Logger> Logger for TestsTreeLogger<'_, P> {
    fn log(&mut self, test: &'static TestCase, update: TestStatusUpdate, elapsed: Duration) {
        match &update {
            TestStatusUpdate::Started { info } => {
                self.is_first_run = info.run_count == 0;
            }
            TestStatusUpdate::Updated { info } => {
                if self.is_first_run {
                    match info {
                        TestUpdate::Verified {
                            message,
                            line_nr,
                            file,
                            outcome,
                        } => {
                            if let VerifyOutcome::Failure { details } = outcome {
                                self.current_failures.push(Failure {
                                    param_index: None,
                                    message: message.clone(),
                                    line_nr: *line_nr,
                                    file,
                                    details: details.clone(),
                                });
                            }
                            // self.current_failures |= !outcome.is_success()
                        }
                        TestUpdate::ParamsStarted {
                            message,
                            line_nr,
                            file,
                            header: _,
                        } => {
                            self.current_param = Some(ParamInfo {
                                message: message.clone(),
                                line_nr: *line_nr,
                                file,
                            });
                        }
                        TestUpdate::ParamVerified {
                            index,
                            row_fields: _,
                            outcome,
                        } => {
                            if let VerifyOutcome::Failure { details } = outcome {
                                let param = self
                                    .current_param
                                    .as_ref()
                                    .expect("ParamVerified update without current param");
                                self.current_failures.push(Failure {
                                    param_index: Some(*index),
                                    message: param.message.clone(),
                                    line_nr: param.line_nr,
                                    file: param.file,
                                    details: details.clone(),
                                });
                            }
                            // self.current_failures |= !outcome.is_success()
                        }
                        TestUpdate::ParamsFinished => {}
                    }
                }
            }
            TestStatusUpdate::Finished { panic_message } => {
                if self.is_first_run {
                    self.current_failures
                        .extend(panic_message.as_ref().map(|message| Failure {
                            param_index: None,
                            message: "test should not panic".to_string(),
                            file: test.filename,
                            line_nr: test.line_nr,
                            details: message.clone(),
                        }));
                    if !self.current_failures.is_empty() {
                        self.summary
                            .failed
                            .push((test.name, take(&mut self.current_failures)));
                    } else {
                        self.summary.passed.push(test.name);
                    }
                }
            }
            TestStatusUpdate::Skipped { info, reason } => {
                // this is a separate test, don't use `is_first_run` flag here
                if info.depth == 0 {
                    self.summary.skipped.push((test.name, reason.clone()));
                }
            }
        }
        self.printer.log(test, update, elapsed)
    }
}

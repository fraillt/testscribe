use crate::processor::logger::{TestUpdate, VerifyOutcome};
use crate::report::TestReport;
use crate::test_args::ParamDisplay;

pub struct CheckReporter<'a> {
    line: u32,
    file: &'static str,
    report: &'a mut TestReport,
}

impl<'a> CheckReporter<'a> {
    pub fn new(line: u32, file: &'static str, report: &'a mut TestReport) -> Self {
        Self { line, file, report }
    }
    pub fn set_outcome(self, message: String, outcome: VerifyOutcome) {
        self.report.update(TestUpdate::Verified {
            message,
            file: self.file,
            line_nr: self.line,
            outcome,
        });
    }

    pub fn into_param_check_reporter<T>(self, message: String) -> ParamCheckReporter<'a>
    where
        T: ParamDisplay + 'static,
    {
        self.report.update(TestUpdate::ParamsStarted {
            message,
            line_nr: self.line,
            file: self.file,
            header: Vec::from(T::NAMES),
        });
        ParamCheckReporter {
            report: self.report,
        }
    }
}

pub struct ParamCheckReporter<'a> {
    report: &'a mut TestReport,
}

impl ParamCheckReporter<'_> {
    pub fn set_param_outcome(
        &mut self,
        index: usize,
        row_fields: Vec<String>,
        outcome: VerifyOutcome,
    ) {
        self.report.update(TestUpdate::ParamVerified {
            index,
            row_fields,
            outcome,
        });
    }
}

impl Drop for ParamCheckReporter<'_> {
    fn drop(&mut self) {
        self.report.update(TestUpdate::ParamsFinished);
    }
}

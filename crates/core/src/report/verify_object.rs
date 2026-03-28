use crate::report::TestReport;
use crate::report::check_report::CheckReporter;

/// Is created with `then!` macro when variable or value expression is provided
/// All fields are private, so they don't appear in IDE suggestions
pub struct VerifyValue<'a, T> {
    var_name: &'static str,
    actual_value: &'a T,
    reporter: CheckReporter<'a>,
}

impl<'a, T> VerifyValue<'a, T> {
    #[must_use = "VerifyValue must be used to report the check outcome"]
    pub fn new(
        report: &'a mut TestReport,
        actual_value: &'a T,
        var_name: &'static str,
        line: u32,
        file: &'static str,
    ) -> Self {
        Self {
            var_name,
            actual_value,
            reporter: CheckReporter::new(line, file, report),
        }
    }
}

/// Same as [VerifyValue], except all fields are public
/// Used in trait implementations to access the fields
pub struct VerifyValueExposed<'a, T> {
    /// Variable name specified via `then!` macro
    pub var_name: &'static str,
    /// A reference to an actual value
    pub actual_value: &'a T,
    /// Reporter object used to report the check outcome
    pub reporter: CheckReporter<'a>,
}

impl<'a, T> VerifyValueExposed<'a, T> {
    pub fn new(value: VerifyValue<'a, T>) -> Self {
        Self {
            var_name: value.var_name,
            actual_value: value.actual_value,
            reporter: value.reporter,
        }
    }
}

/// Is created with `then!` macro when string message is provided
/// All fields are private, so they don't appear in IDE suggestions
pub struct VerifyStatement<'a, const ASYNC: bool> {
    message: &'static str,
    reporter: CheckReporter<'a>,
}

impl<'a, const ASYNC: bool> VerifyStatement<'a, ASYNC> {
    #[must_use = "VerifyStatement must be used to report the check outcome"]
    pub fn new(
        report: &'a mut TestReport,
        message: &'static str,
        line: u32,
        file: &'static str,
    ) -> Self {
        Self {
            message,
            reporter: CheckReporter::new(line, file, report),
        }
    }
}

/// Same as [VerifyStatement], except all fields are public
/// Used in trait implementations to access the fields
pub struct VerifyStatementExposed<'a> {
    /// Variable name specified via `then!` macro
    pub message: &'static str,
    /// Reporter object used to report the check outcome
    pub reporter: CheckReporter<'a>,
}

impl<'a> VerifyStatementExposed<'a> {
    pub fn new<const ASYNC: bool>(value: VerifyStatement<'a, ASYNC>) -> Self {
        Self {
            message: value.message,
            reporter: value.reporter,
        }
    }
}

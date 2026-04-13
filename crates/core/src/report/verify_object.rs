use crate::report::TestReport;
use crate::report::check_report::CheckReporter;

/// Pending check on a value, created by `then!(variable)` or `then!(expression => alias)`.
///
/// `VerifyValue` itself has no check methods; checks are provided by extension traits, such
/// as [`CheckEq`](crate::report::basic::CheckEq) from the `basic` module, or custom traits
/// defined in your own test code (see [`VerifyValueExposed`]).
///
/// All fields are private, so they don't appear in IDE suggestions.
pub struct VerifyValue<'a, T> {
    var_name: &'static str,
    actual_value: &'a T,
    reporter: CheckReporter<'a>,
}

impl<'a, T> VerifyValue<'a, T> {
    /// Called by the generated `then!` macro; not intended to be called directly.
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

/// Same as [`VerifyValue`], except all fields are public.
///
/// Use it to implement custom, domain-specific checks: convert the [`VerifyValue`] you receive,
/// inspect [`actual_value`](VerifyValueExposed::actual_value), and report the result through
/// [`reporter`](VerifyValueExposed::reporter).
///
/// Equality ([`eq`](crate::report::basic::CheckEq)) and containment
/// ([`contains`](crate::report::basic::CheckContains)) already ship as built-in checks; a
/// custom check is for something they don't express. For example, a `sorted_ascending` check
/// for `Vec<T>`, usable as `then!(list).sorted_ascending()`:
///
/// ```
/// use std::fmt::Debug;
/// use testscribe_core::report::{VerifyOutcome, VerifyValue, VerifyValueExposed};
///
/// pub trait CheckSortedAscending {
///     fn sorted_ascending(self);
/// }
///
/// impl<T> CheckSortedAscending for VerifyValue<'_, Vec<T>>
/// where
///     T: PartialOrd + Debug,
/// {
///     fn sorted_ascending(self) {
///         let this = VerifyValueExposed::new(self);
///         let out_of_order = this.actual_value.windows(2).any(|pair| pair[0] > pair[1]);
///         this.reporter.set_outcome(
///             format!("{} is sorted ascending", this.var_name),
///             if out_of_order {
///                 VerifyOutcome::Failure {
///                     details: format!("actual values: {:?}", this.actual_value),
///                 }
///             } else {
///                 VerifyOutcome::Success
///             },
///         );
///     }
/// }
/// ```
pub struct VerifyValueExposed<'a, T> {
    /// Variable name (or alias) specified via `then!` macro
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

/// Pending check on a narrative statement, created by `then!("statement")`.
///
/// `VerifyStatement` itself has no check methods; checks are provided by extension traits
/// from the [`basic`](crate::report::basic) module:
/// [`CheckRun`](crate::report::basic::CheckRun),
/// [`CheckAsyncRun`](crate::report::basic::CheckAsyncRun) and
/// [`CheckParams`](crate::report::basic::CheckParams).
///
/// The `ASYNC` const parameter mirrors whether the enclosing test function is async;
/// `run_async` is only available inside async tests.
///
/// All fields are private, so they don't appear in IDE suggestions.
pub struct VerifyStatement<'a, const ASYNC: bool> {
    message: &'static str,
    reporter: CheckReporter<'a>,
}

impl<'a, const ASYNC: bool> VerifyStatement<'a, ASYNC> {
    /// Called by the generated `then!` macro; not intended to be called directly.
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

/// Same as [`VerifyStatement`], except all fields are public.
///
/// Use it to implement custom checks on statements, the same way [`VerifyValueExposed`]
/// is used for values.
pub struct VerifyStatementExposed<'a> {
    /// Statement message specified via `then!` macro
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

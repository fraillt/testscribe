use std::any::Any;
use std::fmt::Debug;
use std::future::Future;
use std::ops::AsyncFnMut;
use std::panic::{AssertUnwindSafe, catch_unwind};

use futures::FutureExt;

use crate::processor::panic::extract_string_from_panic_payload;
use crate::report::ParamCheckReporter;
use crate::report::VerifyOutcome;
use crate::report::verify_object::{
    VerifyStatement, VerifyStatementExposed, VerifyValue, VerifyValueExposed,
};
use crate::test_args::ParamDisplay;

/// Equality checks for [`VerifyValue`]: `then!(sum).eq(3)`, `then!(sum).ne(5)`.
pub trait CheckEq<T> {
    /// Checks `value == rhs`; reports `<name> is equal to <rhs>`.
    fn eq<T1>(self, rhs: T1)
    where
        T: PartialEq<T1> + Debug,
        T1: Debug;

    /// Checks `value != rhs`; reports `<name> is not equal to <rhs>`.
    fn ne<T1>(self, rhs: T1)
    where
        T: PartialEq<T1> + Debug,
        T1: Debug;
}

impl<T> CheckEq<T> for VerifyValue<'_, T> {
    fn eq<T1>(self, rhs: T1)
    where
        T: PartialEq<T1> + Debug,
        T1: Debug,
    {
        let this = VerifyValueExposed::new(self);
        this.reporter.set_outcome(
            format!("{} is equal to {:?}", this.var_name, rhs),
            if this.actual_value.eq(&rhs) {
                VerifyOutcome::Success
            } else {
                VerifyOutcome::Failure {
                    details: format!("actual: {:?}", this.actual_value),
                }
            },
        );
    }

    fn ne<T1>(self, rhs: T1)
    where
        T: PartialEq<T1> + Debug,
        T1: Debug,
    {
        let this = VerifyValueExposed::new(self);
        this.reporter.set_outcome(
            format!("{} is not equal to {:?}", this.var_name, rhs),
            if this.actual_value.ne(&rhs) {
                VerifyOutcome::Success
            } else {
                VerifyOutcome::Failure {
                    details: format!("actual: {:?}", this.actual_value),
                }
            },
        );
    }
}

/// Containment checks for [`VerifyValue`]: element membership for collections
/// (`then!(items).contains(3)`) and substring matching for text
/// (`then!(message).contains("declined")`).
pub trait CheckContains<Rhs> {
    /// Checks that the value contains `expected`; reports `<name> contains <expected>`.
    ///
    /// For a `Vec<T>` this means an element equal to `expected`; for a `String` / `&str` it
    /// means `expected` occurs as a substring.
    fn contains(self, expected: Rhs);
}

/// Membership check: passes when some element of the `Vec` equals `expected`. The argument
/// type is independent of the element type (like [`CheckEq::eq`]), so a `Vec<String>` can be
/// probed with a `&str`: `then!(tags).contains("urgent")`.
impl<T, Rhs> CheckContains<Rhs> for VerifyValue<'_, Vec<T>>
where
    T: PartialEq<Rhs> + Debug,
    Rhs: Debug,
{
    fn contains(self, expected: Rhs) {
        let this = VerifyValueExposed::new(self);
        let found = this.actual_value.iter().any(|item| *item == expected);
        this.reporter.set_outcome(
            format!("{} contains {:?}", this.var_name, expected),
            if found {
                VerifyOutcome::Success
            } else {
                VerifyOutcome::Failure {
                    details: format!("actual: {:?}", this.actual_value),
                }
            },
        );
    }
}

/// Substring check for an owned `String`.
impl CheckContains<&str> for VerifyValue<'_, String> {
    fn contains(self, expected: &str) {
        let this = VerifyValueExposed::new(self);
        report_substring(this, expected);
    }
}

/// Substring check for a borrowed `&str`.
impl CheckContains<&str> for VerifyValue<'_, &str> {
    fn contains(self, expected: &str) {
        let this = VerifyValueExposed::new(self);
        report_substring(this, expected);
    }
}

/// Shared reporting for the string-substring `contains` impls.
fn report_substring<S>(this: VerifyValueExposed<'_, S>, expected: &str)
where
    S: AsRef<str> + Debug,
{
    let found = this.actual_value.as_ref().contains(expected);
    this.reporter.set_outcome(
        format!("{} contains {:?}", this.var_name, expected),
        if found {
            VerifyOutcome::Success
        } else {
            VerifyOutcome::Failure {
                details: format!("actual: {:?}", this.actual_value),
            }
        },
    );
}

/// Closure check for [`VerifyStatement`]: `then!("payment is accepted").run(|| ...)`.
pub trait CheckRun {
    /// Runs the closure and reports the statement as the check outcome.
    ///
    /// The check fails if the closure panics (panics are caught), returns `false`,
    /// or returns an `Err`; see the [`VerifyOutcome`] `From` implementations.
    fn run<Res>(self, test_fn: impl FnOnce() -> Res)
    where
        Res: Into<VerifyOutcome>;
}

impl<const ASYNC: bool> CheckRun for VerifyStatement<'_, ASYNC> {
    fn run<Res>(self, test_fn: impl FnOnce() -> Res)
    where
        Res: Into<VerifyOutcome>,
    {
        let res = catch_unwind(AssertUnwindSafe(test_fn));
        let this = VerifyStatementExposed::new(self);
        this.reporter
            .set_outcome(this.message.to_owned(), get_run_outcome(res));
    }
}

/// Async closure check for [`VerifyStatement`]:
/// `then!("payment is accepted").run_async(async || ...).await`.
///
/// Only available inside async test functions.
pub trait CheckAsyncRun {
    /// Async counterpart of [`CheckRun::run`]; same failure rules apply.
    fn run_async<Res>(self, test_fn: impl AsyncFnOnce() -> Res) -> impl Future<Output = ()>
    where
        Res: Into<VerifyOutcome>;
}

impl CheckAsyncRun for VerifyStatement<'_, true> {
    async fn run_async<Res>(self, test_fn: impl AsyncFnOnce() -> Res)
    where
        Res: Into<VerifyOutcome>,
    {
        let res = AssertUnwindSafe(test_fn()).catch_unwind().await;
        let this = VerifyStatementExposed::new(self);
        this.reporter
            .set_outcome(this.message.to_owned(), get_run_outcome(res));
    }
}

/// Pending table-style check, created by [`CheckParams::params`]; finish it with
/// [`run`](VerifyParamsRun::run) or [`run_async`](VerifyParamsRun::run_async).
pub struct VerifyParamsRun<'a, T, const ASYNC: bool>
where
    T: ParamDisplay + 'static,
{
    reporter: ParamCheckReporter<'a>,
    params: Vec<T>,
}

/// Table-style check for [`VerifyStatement`]: runs the same closure for every item in a list
/// and reports each item as a table row in the test output.
///
/// ```ignore
/// then!("check adding one")
///     .params(vec![
///         TestCase { input: 1, expected: 2 },
///         TestCase { input: 2, expected: 3 },
///     ])
///     .run(|test_case| add_one(test_case.input) == test_case.expected);
/// ```
///
/// produces:
/// ```text
/// |       -|  Then check adding one
/// |       -|  | input, expected |
/// |       -|  |     1,        2 |
/// |       -|  |     2,        3 |
/// ```
pub trait CheckParams<'a, const ASYNC: bool> {
    /// Provides the list of items to check; each item becomes one output row,
    /// rendered via its [`ParamDisplay`] implementation.
    fn params<T>(self, list: impl IntoIterator<Item = T>) -> VerifyParamsRun<'a, T, ASYNC>
    where
        T: ParamDisplay + 'static;
}

impl<'a, const ASYNC: bool> CheckParams<'a, ASYNC> for VerifyStatement<'a, ASYNC> {
    fn params<T>(self, list: impl IntoIterator<Item = T>) -> VerifyParamsRun<'a, T, ASYNC>
    where
        T: ParamDisplay + 'static,
    {
        let this = VerifyStatementExposed::new(self);
        VerifyParamsRun {
            reporter: this
                .reporter
                .into_param_check_reporter(this.message.to_owned(), Vec::from(T::NAMES)),
            params: list.into_iter().collect(),
        }
    }
}

impl<T, const ASYNC: bool> VerifyParamsRun<'_, T, ASYNC>
where
    T: ParamDisplay + 'static,
{
    /// Runs the closure once per item; per-item failure rules are the same as in
    /// [`CheckRun::run`].
    pub fn run<Res>(mut self, mut test_fn: impl FnMut(T) -> Res)
    where
        Res: Into<VerifyOutcome>,
    {
        for (index, value) in self.params.into_iter().enumerate() {
            let row_fields = value.values();
            let res = catch_unwind(AssertUnwindSafe(|| test_fn(value)));
            self.reporter
                .set_param_outcome(index, row_fields, get_run_outcome(res));
        }
    }
}

impl<T> VerifyParamsRun<'_, T, true>
where
    T: ParamDisplay + 'static,
{
    /// Async counterpart of [`VerifyParamsRun::run`]; only available inside async tests.
    pub async fn run_async<Res>(mut self, mut test_fn: impl AsyncFnMut(T) -> Res)
    where
        Res: Into<VerifyOutcome>,
    {
        for (index, value) in self.params.into_iter().enumerate() {
            let row_fields = value.values();
            let res = AssertUnwindSafe(test_fn(value)).catch_unwind().await;
            self.reporter
                .set_param_outcome(index, row_fields, get_run_outcome(res));
        }
    }
}

fn get_run_outcome<Res>(res: Result<Res, Box<dyn Any + Send>>) -> VerifyOutcome
where
    Res: Into<VerifyOutcome>,
{
    match res {
        Ok(res) => res.into(),
        Err(err) => VerifyOutcome::Failure {
            details: format!(
                "panic: {}",
                extract_string_from_panic_payload(&err).unwrap_or_else(|| "no info".to_owned())
            ),
        },
    }
}

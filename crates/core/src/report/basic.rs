use std::any::Any;
use std::error::Error;
use std::fmt::Debug;
use std::future::Future;
use std::ops::AsyncFnMut;
use std::panic::{AssertUnwindSafe, catch_unwind};

use futures::FutureExt;

use crate::processor::logger::VerifyOutcome;
use crate::processor::panic::extract_string_from_panic_payload;
use crate::report::ParamCheckReporter;
use crate::report::verify_object::{
    VerifyStatement, VerifyStatementExposed, VerifyValue, VerifyValueExposed,
};
use crate::test_args::ParamDisplay;

pub trait CheckEq<T> {
    fn eq<T1>(self, rhs: T1)
    where
        T: PartialEq<T1> + Debug,
        T1: Debug;

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

impl From<()> for VerifyOutcome {
    fn from(_value: ()) -> Self {
        VerifyOutcome::Success
    }
}

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

pub trait CheckRun {
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

pub trait CheckAsyncRun {
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

pub struct VerifyParamsRun<'a, T, const ASYNC: bool>
where
    T: ParamDisplay + 'static,
{
    reporter: ParamCheckReporter<'a>,
    params: Vec<T>,
}

pub trait CheckParams<'a, const ASYNC: bool> {
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
                .into_param_check_reporter::<T>(this.message.to_owned()),
            params: list.into_iter().collect(),
        }
    }
}

impl<T, const ASYNC: bool> VerifyParamsRun<'_, T, ASYNC>
where
    T: ParamDisplay + 'static,
{
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

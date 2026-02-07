use std::fmt::Debug;
use std::process::ExitCode;
use testscribe::processor::logger::VerifyOutcome;
use testscribe::report::basic::{CheckEq, CheckRun};
use testscribe::report::{VerifyValue, VerifyValueExposed};
use testscribe::test_args::Given;
use testscribe::{CASES, testscribe};
use testscribe_standalone::args::Arguments;
use testscribe_standalone::run_all_sync;

pub trait CheckContains<T> {
    fn contains(self, value: T)
    where
        T: PartialEq + Debug;
}

impl<T> CheckContains<T> for VerifyValue<'_, Vec<T>> {
    fn contains(self, value: T)
    where
        T: PartialEq + Debug,
    {
        let this = VerifyValueExposed::new(self);
        this.reporter.set_outcome(
            format!("`{}` contains {:?}", this.var_name, value),
            if this.actual_value.contains(&value) {
                VerifyOutcome::Success
            } else {
                VerifyOutcome::Failure {
                    details: format!("actual values: {:?}", this.actual_value),
                }
            },
        );
    }
}

#[testscribe(cloneable)]
fn two_simple_words() {
    let xxx = 5;
    then!("custom message").run(|| {});
    then!(xxx).eq(5);
}

#[testscribe(tags=[fast, boo])]
fn custom_check_value_trait() -> u64 {
    let xxx = vec![5];
    then!(xxx).contains(5);
    xxx[0]
}

#[testscribe(cloneable, tags=[slow, ignore])]
fn boo1(Given(a): Given<Boo>) -> u64 {
    then!(a).eq(3);
    then!(a).ne(6);
    a
}

#[testscribe(cloneable)]
fn boo3(Given(boo): Given<Boo1>) -> u64 {
    then!(boo).eq(4);
    boo
}

#[testscribe(cloneable)]
fn boo2(Given(boo): Given<Boo>) -> u64 {
    then!(boo).eq(4);
    boo
}

#[testscribe()]
fn boo() -> u64 {
    then!("");
    4
}

fn main() -> ExitCode {
    run_all_sync(&CASES, Arguments::from_args())
        .unwrap()
        .exit_code()
}

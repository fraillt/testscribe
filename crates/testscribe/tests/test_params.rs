mod utils;

use std::u8;

use testscribe::ParamDisplay;
use testscribe::report::basic::{CheckParams, CheckRun};
use testscribe::test_args::{Given, Param};
use testscribe::testscribe;

fn yes_no(value: &bool) -> String {
    if *value { "yes" } else { "no" }.to_string()
}

#[derive(Debug, Clone, ParamDisplay)]
struct AgeVerify {
    age: i32,
    is_old: bool,
}

#[testscribe(params)]
fn p1() -> Vec<AgeVerify> {
    vec![
        AgeVerify {
            age: 25,
            is_old: true,
        },
        AgeVerify {
            age: 9,
            is_old: false,
        },
    ]
}

#[derive(Debug, Clone, Copy, ParamDisplay)]
pub struct IsGood {
    #[pd(custom=yes_no)]
    is_good: bool,
}

#[testscribe(params)]
pub fn p2() -> Vec<IsGood> {
    [true, false]
        .into_iter()
        .map(|is_good| IsGood { is_good })
        .collect()
}

#[testscribe(standalone)]
#[test]
fn root_test(_: Param<P1>) -> u8 {
    then!("fadsfay").run(|| {});
    5
}

#[testscribe(cloneable)]
fn some_test1(_arg1: Given<RootTest>, _arg2: Param<P1>) {
    then!("fadsfay").run(|| {});
}

#[testscribe(cloneable)]
fn some_test2(_arg1: Given<RootTest>, _arg2: Param<P2>) {
    then!("fadsfay").run(|| {});
}

fn my_display(v: &u16) -> String {
    (v + 5).to_string()
}

#[derive(Debug, Clone, ParamDisplay)]
pub struct User {
    #[pd(debug)]
    name: String,
    #[pd(custom=my_display)]
    age: u16,
}

#[testscribe(standalone)]
#[test]
fn then_clause_accepts_arbitrary_params() {
    then!("check these")
        .params([
            User {
                name: "Tomas".to_string(),
                age: 54,
            },
            User {
                name: "Jonas".to_string(),
                age: 71,
            },
        ])
        .run(|_p| {});
}

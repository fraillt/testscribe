use std::time::Duration;

use testscribe::ParamDisplay;
use testscribe::report::basic::{CheckEq, CheckParams, CheckRun};
use testscribe::test_args::{Given, Param};
use testscribe::testscribe;

#[testscribe(standalone)]
#[test]
fn my_test() -> i32 {
    let x = 4;
    then!(x).eq(4);
    x
}

#[testscribe]
fn run_other_test(Given(x): Given<MyTest>) {
    then!(x).eq(4);
}

#[testscribe(standalone)]
#[test]
fn panicking_test() -> i32 {
    let x = 4;
    then!(x).eq(4);
    x
}

#[testscribe]
fn some_other_test(Given(x): Given<PanickingTest>) {
    then!(x).ne(5);
}

#[testscribe(standalone)]
#[tokio::test]
async fn async_test() -> i32 {
    tokio::time::sleep(Duration::from_millis(10)).await;
    then!("start of test").run(|| {});
    4
}

#[derive(Debug, Clone, ParamDisplay)]
struct NameWithCount {
    count: i32,
    name: &'static str,
}

#[testscribe(params)]
fn p1() -> Vec<NameWithCount> {
    [(3, "boo"), (4, "moo"), (5, "foo"), (6, "foo")]
        .into_iter()
        .map(|(count, name)| NameWithCount { count, name })
        .collect()
}

#[testscribe(tags = [ignored])]
async fn depends_on_async_must_be_also_async(_x: Given<AsyncTest>, Param(p): Param<P1>) {
    if p.count == 3 {
        then!(p.count => count).eq(3);
    } else {
        then!(p.count => count).ne(3);
    }
}

#[testscribe]
async fn depends_on_async_must_be_also_async2(_x: Given<AsyncTest>) {
    then!("create multiple contracts")
        .params(p1())
        .run_async(async |p| assert_ne!(p.count, 10))
        .await;
}

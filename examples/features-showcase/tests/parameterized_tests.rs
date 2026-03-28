use testscribe::{
    ParamDisplay,
    report::basic::CheckEq,
    test_args::{Given, Param},
    testscribe,
};

/// Define a struct used as a test parameter.
/// It must implement `ParamDisplay` so the parameter can be shown in the test report.
#[derive(Clone, ParamDisplay)]
struct TestCaseParam {
    nr: i32,
}

/// Define parameters for a parameterized test.
/// This behaves similarly to a test function, by defining a type using function name but converted to PascalCase,
/// `TestParams` in this case, and it needs to return a vector of parameters, which will be used to run the test for each parameter.
#[testscribe(params)]
fn test_params() -> Vec<TestCaseParam> {
    vec![TestCaseParam { nr: 1 }, TestCaseParam { nr: 2 }]
}

/// Add `Param<TestParams>` to the test function parameters.
/// The test runs once for each parameter returned by `test_params`.
/// Running this test will give this outcome:
/// ```text
/// | 6.900μs|Given add numbers
/// |        |With nr=1
/// |       -|  Then nr is equal to 1
/// | 2.030μs|  When child test executed for each param
/// |       -|    Then state is equal to 1
/// | 1.160μs|Given add numbers
/// |        |With nr=2
/// |       -|  Then nr is equal to 2
/// | 1.170μs|  When child test executed for each param
/// |       -|    Then state is equal to 2
///```
#[testscribe(standalone)]
#[test]
fn add_numbers(p: Param<TestParams>) -> i32 {
    then!(p.nr => nr).eq(p.nr);
    p.nr
}

#[testscribe]
fn child_test_executed_for_each_param(state: Given<AddNumbers>) {
    then!(*state => state).eq(*state);
}

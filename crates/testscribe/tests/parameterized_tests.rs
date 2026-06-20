use testscribe::prelude::*;

/// Define a struct used as a test parameter.
/// It must implement `ParamDisplay` so the parameter can be shown in the test report.
#[derive(Clone, ParamDisplay)]
struct TestCaseParam {
    nr: i32,
    #[pd(hide)]
    extra_data: bool, // This field is used in the test, but we don't want to show it in the test report
}

/// Define parameters for a parameterized test.
/// This behaves similarly to a test function, by defining a type using function name but converted to PascalCase,
/// `TestParams` in this case, and it needs to return a vector of parameters, which will be used to run the test for each parameter.
#[testscribe(params)]
fn test_params() -> Vec<TestCaseParam> {
    vec![
        TestCaseParam {
            nr: 1,
            extra_data: true,
        },
        TestCaseParam {
            nr: 2,
            extra_data: false,
        },
    ]
}

/// Add `Param<TestParams>` to the test function parameters.
/// The test runs once for each parameter returned by `test_params`.
/// Running this test will give this outcome:
/// ```text
/// | 5.771μs|Given add numbers
/// |        |With nr=1
/// |       -|  Then nr is equal to 1
/// | 2.600μs|  When child test executed for each param
/// |       -|    Then extra_data is equal to true
/// | 1.330μs|Given add numbers
/// |        |With nr=2
/// |       -|  Then nr is equal to 2
/// | 1.370μs|  When child test executed for each param
/// |       -|    Then extra_data is equal to false
///```
#[testscribe(standalone)]
fn add_numbers(p: Param<TestParams>) -> bool {
    then!(p.nr => nr).eq(p.nr);
    p.extra_data
}

#[testscribe]
fn child_test_executed_for_each_param(state: Given<AddNumbers>) {
    then!(*state => extra_data).eq(*state);
}

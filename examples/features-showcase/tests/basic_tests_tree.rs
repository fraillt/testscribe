use features_showcase::{add_numbers, divide_by};
use testscribe::{
    report::basic::{CheckEq, CheckRun},
    test_args::Given,
    testscribe,
};

/// Annotate test functions with `testscribe`.
/// If tests are run with the standard libtest runner, root tests need the `standalone` tag
/// so they automatically run all child tests as well.
/// Every test defined with `testscribe` also generates a PascalCase type that child tests can use.
/// In this case, `one_and_two_is_added` generates `OneAndTwoIsAdded`, which `another_6_is_added`
/// uses to access the parent result for additional checks.
/// Running this test will give this outcome:
/// ```text
/// | 7.360μs|Given one and two is added
/// |       -|  Then sum is equal to 3
/// | 1.860μs|  When another 2 is added
/// |       -|    Then sum is equal to 9
/// | 1.000μs|  When another 6 is added
/// |       -|    Then sum is equal to 9
/// | 1.070μs|    When divided by 2
/// |       -|      Then result is equal to 4
/// ```
#[testscribe(standalone)]
#[test]
fn one_and_two_is_added() -> i32 {
    let sum = add_numbers(1, 2);
    then!(sum).eq(3);
    sum
}

/// This child test depends on `one_and_two_is_added`, which is its parent.
/// It will automatically get the result of `one_and_two_is_added` test as a parameter.
/// In order to run this test, you need to run its parent test `one_and_two_is_added`,
/// which will automatically run all child tests as well.
#[testscribe]
fn another_2_is_added(state: Given<OneAndTwoIsAdded>) {
    // `Given` implements `Deref` trait, so you can get the value of parent test by dereferencing it.
    let sum = add_numbers(*state, 6);
    then!(sum).eq(9);
}

/// Because the parent has two child tests and each child needs the parent result,
/// the parent test is executed for each child test.
#[testscribe]
fn another_6_is_added(mut state: Given<OneAndTwoIsAdded>) -> i32 {
    // you can directly modify the value of parent test by dereferencing `Given` and assigning to it.
    *state = add_numbers(*state, 6);
    then!(*state => sum).eq(9);
    // return modified state by converting `Given` to inner value using `into_inner` method
    state.into_inner()
}

#[testscribe]
fn divided_by_2(state: Given<Another6IsAdded>) {
    then!(divide_by(*state, 2) => result).eq(4);
}

/// Async tests are supported as well.
/// Running this test will give this outcome:
/// ```text
/// | 4.080μs|Given async test
/// | 1.320μs|  When child must be async as well
/// |       -|    Then async_child_state is equal to true
/// ```
#[testscribe(standalone)]
#[tokio::test]
async fn async_test() -> bool {
    // no assertion, but just to suppress the "must be used" warning
    then!("it returns true").run(|| {});
    true
}

/// This test demonstrates that if parent test is async, then child test must be async as well.
#[testscribe]
async fn child_must_be_async_as_well(state: Given<AsyncTest>) {
    then!(*state => async_child_state).eq(true);
}

// This file is the reference for the built-in check forms, so it imports each check trait
// explicitly to show which trait powers which `then!` form: `CheckEq` -> `.eq`/`.ne`,
// `CheckContains` -> `.contains`, `CheckRun` -> `.run`, `CheckAsyncRun` -> `.run_async`,
// `CheckParams` -> `.params`. Real test suites don't list these — `use testscribe::prelude::*;`
// re-exports all of them at once (see basic_tests_tree.rs).
use testscribe::report::basic::{CheckAsyncRun, CheckContains, CheckEq, CheckParams, CheckRun};
use testscribe::{ParamDisplay, testscribe};

/// Running this test will give this outcome:
/// ```text
/// | 0.011ms|Given simple value
/// |       -|  Then value is equal to 5
/// ```
#[testscribe(standalone)]
fn simple_value() {
    let value = 5;
    // This macro returns a `VerifyValue` struct. It is opaque to users, but captures
    // the value and variable name for trait-based checks.
    // `CheckEq` is implemented for `VerifyValue`, so we can call `eq` to compare values.
    then!(value).eq(5);
}

/// Running this test will give this outcome:
/// ```text
/// | 9.820μs|Given simple expression
/// |       -|  Then added_values is equal to 7
/// |       -|  And added_values is not equal to 5
/// ```
#[testscribe(standalone)]
fn simple_expression() {
    let value = 5;
    let another_value = 2;
    // This macro also returns a `VerifyValue` struct. It captures the full expression
    // and lets you provide an alias with the `=>` operator.
    then!(value + another_value => added_values).eq(7);
    then!(value + another_value => added_values).ne(5);
}

struct User {
    age: i32,
    name: String,
}

/// Running this test will give this outcome:
/// ```text
/// | 0.013ms|Given field access expression
/// |       -|  Then user_name is equal to "hello"
/// |       -|  And user_age is equal to 5
/// ```
#[testscribe(standalone)]
fn field_access_expression() {
    let value = User {
        age: 5,
        name: "hello".to_string(),
    };
    then!(value.name => user_name).eq("hello");
    then!(value.age => user_age).eq(5);
}

/// The built-in `contains` check covers both collection membership and substring matching.
/// The argument type is independent of the element type, so a `Vec<String>` can be probed with
/// a `&str` (just like `eq`).
/// Running this test will give this outcome:
/// ```text
/// | 0.012ms|Given containment checks
/// |       -|  Then numbers contains 2
/// |       -|  And tags contains "urgent"
/// |       -|  And message contains "declined"
/// ```
#[testscribe(standalone)]
fn containment_checks() {
    let numbers = vec![1, 2, 3];
    then!(numbers).contains(2);

    let tags = vec!["urgent".to_string(), "billing".to_string()];
    then!(tags).contains("urgent");

    let message = "payment was declined".to_string();
    then!(message).contains("declined");
}

/// Running this test will give this outcome:
/// ```text
/// | 0.010ms|Given closure execution
/// |       -|  Then success if do not panic
/// |       -|  And success if returns true
/// |       -|  And success if returns Ok
/// ```
#[testscribe(standalone)]
fn closure_execution() {
    fn get_user_name() -> String {
        "hello".to_string()
    }
    then!("success if do not panic").run(|| {
        let name = get_user_name();
        assert_eq!(name, "hello");
    });
    then!("success if returns true").run(|| {
        let name = get_user_name();
        name == "hello"
    });
    then!("success if returns Ok").run(|| {
        let name = get_user_name();
        Result::<String, std::io::Error>::Ok(name)
    });
}

/// Running this test will give this outcome:
/// ```text
/// | 9.801μs|Given async closure execution
/// |       -|  Then success if do not panic
/// |       -|  And success if returns true
/// |       -|  And success if returns Ok
/// ```
#[testscribe(standalone(tokio::test))]
async fn async_closure_execution() {
    async fn get_user_name() -> String {
        "hello".to_string()
    }
    then!("success if do not panic")
        .run_async(async || {
            let name = get_user_name().await;
            assert_eq!(name, "hello");
        })
        .await;
    then!("success if returns true")
        .run_async(async || {
            let name = get_user_name().await;
            name == "hello"
        })
        .await;
    then!("success if returns Ok")
        .run_async(async || {
            let name = get_user_name().await;
            Result::<String, std::io::Error>::Ok(name)
        })
        .await;
}

/// Running this test will give this outcome:
/// ```text
/// | 0.015ms|Given list of params to check
/// |       -|  Then check adding one
/// |       -|  | input, expected |
/// |       -|  |     1,        2 |
/// |       -|  |     2,        3 |
/// |       -|  |     3,        4 |
/// ```
#[testscribe(standalone)]
fn list_of_params_to_check() {
    fn add_one(x: i32) -> i32 {
        x + 1
    }

    #[derive(Debug, ParamDisplay, Clone)]
    struct TestCase {
        input: i32,
        expected: i32,
    }

    then!("check adding one")
        .params(vec![
            TestCase {
                input: 1,
                expected: 2,
            },
            TestCase {
                input: 2,
                expected: 3,
            },
            TestCase {
                input: 3,
                expected: 4,
            },
        ])
        .run(|test_case| add_one(test_case.input) == test_case.expected);
}

/// Running this test will give this outcome:
/// ```text
/// | 0.012ms|Given list of params to async check
/// |       -|  Then check adding one
/// |       -|  | input, expected |
/// |       -|  |     1,        2 |
/// |       -|  |     2,        3 |
/// |       -|  |     3,        4 |
/// ```
#[testscribe(standalone(tokio::test))]
async fn list_of_params_to_async_check() {
    async fn add_one(x: i32) -> i32 {
        x + 1
    }

    #[derive(Debug, ParamDisplay, Clone)]
    struct TestCase {
        input: i32,
        expected: i32,
    }

    then!("check adding one")
        .params(vec![
            TestCase {
                input: 1,
                expected: 2,
            },
            TestCase {
                input: 2,
                expected: 3,
            },
            TestCase {
                input: 3,
                expected: 4,
            },
        ])
        .run_async(async |test_case| {
            let res = add_one(test_case.input).await;
            assert_eq!(res, test_case.expected);
        })
        .await;
}

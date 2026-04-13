These tests showcase framework features and how to use them. Each file is a complete,
runnable example of one feature, together with the test output it produces.

IMPORTANT: These examples focus only on the syntax, DO NOT use them as examples how to name things.

## Basic Usage

### Test organization and assertion

- [Basic test tree](./basic_tests_tree.rs) - build a test tree with parent and child test functions.
- [Environment](./environment.rs) - define an environment shared across tests in a tree.
- [Basic checks](./basic_checks.rs) - write checks (assertions) inside tests.
- [Custom checks](./custom_checks.rs) - define custom check helpers for your test state.

## Advanced techniques

- [Custom test runner](./custom_test_runner.rs) - use a custom test runner instead of the standard Rust `libtest` runner.
- [Clone state and environment](./clone_state_and_environment.rs) - clone test state and environment so parent tests do not need to rerun for each child.
- [Parameterized tests](./parameterized_tests.rs) - run the same test logic with multiple parameters.

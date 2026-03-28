This is a minimal project that showcases framework features and how to use them.

IMPORTANT: These examples focuses only on the syntax, DO NOT use them as examples how to name things.

## Basic Usage

### Test organization and assertion

- [Basic test tree](tests/basic_tests_tree.rs) - build a test tree with parent and child test functions.
- [Environment](tests/environment.rs) - define an environment shared across tests in a tree.
- [Basic checks](tests/basic_checks.rs) - write checks (assertions) inside tests.
- [Custom checks](tests/custom_checks.rs) - define custom check helpers for your test state.

## Advanced techniques

- [Custom test runner](tests/custom_test_runner.rs) - use a custom test runner instead of the standard Rust `libtest` runner.
- [Clone state and environment](tests/clone_state_and_environment.rs) - clone test state and environment so parent tests do not need to rerun for each child.
- [Parameterized tests](tests/parameterized_tests.rs) - run the same test logic with multiple parameters.

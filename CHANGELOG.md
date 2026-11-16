# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

This changelog covers all crates in the workspace
(`testscribe`, `testscribe-core`, `testscribe-proc-macros`, `testscribe-standalone`,
`testscribe-detached`), which are versioned and released together.

<!-- next-header -->

## [Unreleased] - ReleaseDate

### Added

- built-in checks `is_empty` and `is_not_empty` for collections and texts:
  `then!(sent_emails).is_empty()` reports `Then sent emails is empty`.
- built-in predicate checks `is` and `is_not`, for what a value is rather than what it equals,
  such as the variant of an enum with a payload:
  `then!(step).is("a rollback", |step| matches!(step, Step::RolledBack { .. }))` reports
  `Then step is a rollback`, and shows the actual value when it fails.
- built-in check `is_rejected` for an action that has to be refused:
  `then!(paying_again).is_rejected("order is already paid")` passes for an error that reads
  exactly that, and reports `Then paying again is rejected: order is already paid`.
- `TESTSCRIBE_ONLY=<test name> cargo test` runs only the path from the root to that test, for a
  test anywhere in a standalone tree. Naming a test below the root to `cargo test` itself runs
  nothing, because the harness only knows the roots.
- `detached`: `TESTSCRIBE_BATCH_ID=<any text>` names the batch a run belongs to, and the test
  binary tells it to the frontend. The frontend in `frontend/` stores named batches in SQLite,
  compares two of them by what their tests say (added, deleted and modified tests), and remembers
  per batch what to run the next time: everything, the tests selected in the UI, or the new and
  the failed ones.

### Fixed

- a test tree passed although its tests could not run. A panic while an environment was created,
  or while state or environment was cloned for the next test, only skipped the tests below; nothing
  was reported as failed, and `cargo test` printed `ok`. The test that needed the environment or
  the clone is now reported as failed.
- a test that passed on its first run and panicked when executed again for a later branch did not
  fail the tree. It does now, and is printed again where it failed.
- panic messages were lost: a panicking test was reported with `<unknown msg>` and a panicking
  check with `panic: no info`.
- `--exact` with the `exact_this` tag ran the whole tree when the tagged test was a leaf. It now
  runs only the path to that test, as it did for a test with children.
- a root test without `standalone` was run by nothing under the standard test harness, and
  neither were the tests below it, while `cargo test` printed `ok`. Such a root now shows up as
  an ignored test that says why, `ignored, a root test without standalone is not run by cargo test`.
  With `harness = false` nothing changes.
- the tests above an ignored test were executed again only to have their result thrown away,
  once for every ignored branch. With one environment per path, running a single path of a tree
  took almost as long as running all of it. An ignored test no longer has anything built for it.

### Changed

- the experimental `detached` feature is no longer a runner of its own. The tests run under
  `cargo test` as always, and with `TESTSCRIBE_FRONTEND=<host:port>` set every standalone tree
  reports to a frontend: the test binary sends its trees, each tree asks what to run, and every
  test is reported as it runs. Without the variable, or with nothing listening, nothing changes.
  The former `driver` and `runtime` modules are gone. A first frontend, a server with a web UI,
  is in `frontend/`.
- a variable name or alias is printed with spaces for its underscores, the way a test name is:
  `then!(keyboard_stock).eq(5)` reports `Then keyboard stock is equal to 5`, where it was
  `Then keyboard_stock is equal to 5`. `VerifyValueExposed::var_name` holds the printed form.

- with `--nocapture`, a standalone tree is printed in one piece when it has finished, so the lines
  of trees running in parallel no longer mix. It still prints test by test when it runs alone
  (`--test-threads=1`).
- a test that failed or was skipped is no longer executed again for every test below it. An
  environment that waits 30 seconds for a database that is down is waited for once, not once per
  leaf of the tree.

## [0.2.0] - 2026-06-22

### Added

- new field attribute `#[pd(hide)]` on the `ParamDisplay` derive macro. It hides a field from the test output.
- `ParamCheckReporter` initialization without a header.

### Fixed

- reset text styles in the standalone test printer instead of setting them to white.

## [0.1.0] - 2026-06-16

### Added

- `testscribe` testing framework for stateful systems: tests build on each other's state, forming a
  **test tree** with readable Given/When/Then output.
- `#[testscribe]` attribute macro for declaring test nodes, with `Given<Parent>` arguments to build
  on a parent test's state.
- `then!` assertion macro and basic reporting types.
- `standalone` runner (default feature): integrates test trees with the standard test harness, so a
  whole tree runs under `cargo test`. Building blocks are also exposed for custom test runners.
- `detached` runner (experimental, opt-in feature): remote-controlled test execution.
- `#[derive(CloneAsync)]` and `ParamDisplay` derive macros.
- Documentation guides shipped inside the published `testscribe` crate, and a Claude Code plugin /
  `testscribe` skill for AI-agent discoverability.

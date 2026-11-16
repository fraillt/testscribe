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
  `then!(sent_emails).is_empty()` reports `Then sent_emails is empty`.
- built-in predicate checks `is` and `is_not`, for what a value is rather than what it equals,
  such as the variant of an enum with a payload:
  `then!(step).is("a rollback", |step| matches!(step, Step::RolledBack { .. }))` reports
  `Then step is a rollback`, and shows the actual value when it fails.

### Fixed

- a test tree passed although its tests could not run. A panic while an environment was created,
  or while state or environment was cloned for the next test, only skipped the tests below; nothing
  was reported as failed, and `cargo test` printed `ok`. The test that needed the environment or
  the clone is now reported as failed.
- a test that passed on its first run and panicked when executed again for a later branch did not
  fail the tree. It does now, and is printed again where it failed.
- panic messages were lost: a panicking test was reported with `<unknown msg>` and a panicking
  check with `panic: no info`.

### Changed

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

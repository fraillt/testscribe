# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

This changelog covers all crates in the workspace
(`testscribe`, `testscribe-core`, `testscribe-proc-macros`, `testscribe-standalone`,
`testscribe-detached`), which are versioned and released together.

<!-- next-header -->

## [Unreleased] - ReleaseDate

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

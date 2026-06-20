# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

This changelog covers all crates in the workspace
(`testscribe`, `testscribe-core`, `testscribe-proc-macros`, `testscribe-standalone`,
`testscribe-detached`), which are versioned and released together.

<!-- next-header -->

## [Unreleased] - ReleaseDate

## new changes

- added `#[pd(hide)]` on `ParamDisplay`, to hide fields from displaying. This is very useful when using `parameterized_tests`, as it allows to pass more richer data to the test, but do not show everything on one line, but display/use it later. This attribute also works when `then!("").params` as well.
- fixed standalone printed, by reseting text style, instead of setting it to white. It looked weird when some lines had no style, other were white.
- `CheckReporter` is not consumed when calling `set_outcome`, additionally `ParamCheckReporter` can be converted back to `CheckReporter`. Since `then!` cannot be captured from test function (which is deliberate choice, because I want to see assertion in the test function on in some helper functions) this would allow more freedom in custom checks implementations, by allowing to perform multiple checks in a single check command. e.g. `then!(payment_transaction).transfers(Account1, Account2, 50.5, "EUR")` could print/check 3 lines debit, credit and amount.

## feedback applicable to llms/agents?

- 1. Document the discovery workflow — this is your hidden superpower. Because checks record rather than panic, one --nocapture run prints actual: … for every wrong cell. I filled a 9-row × 7-column matrix in basically two runs: write placeholders → run → paste the actuals. That is a genuinely great authoring loop and it's completely undocumented (I only found it by accident). A short "writing matrices by discovery" guide — possibly with a mode that emits copy-pasteable expected values — would make matrix authoring near-mechanical.

- 2. A "rich behavior matrix" pattern page. does_it_fit.md shows the simple matrix (name/accepted). The shape I converged on — each row carrying expected outputs for several uniform checks — isn't shown anywhere, yet it's the obvious destination for stateful systems. Pair it with #[pd(skip)] and it becomes the canonical pattern.

- 3. Elevate "story vs. matrix" and warn about the tree trap. The single most useful guidance was does_it_fit.md's "is this a linear story or an enumerable matrix?" — both times I shoehorned this session, it was because I'd drifted from it (forcing independent reconcile cases into a Given → When → When lineage). That question deserves to be near the top of foundations.md/llms.md, with an explicit caution: testscribe is seductive enough that you want to make everything a tree, and some things just aren't.

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

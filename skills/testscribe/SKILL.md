---
name: testscribe
description: Write or review Rust tests using the testscribe test framework (test trees with Given/When/Then output). Use when a Rust project depends on the testscribe crate and the task involves writing, changing, or reviewing tests.
---

# Writing testscribe tests

testscribe builds **test trees**: each test performs ONE business action, asserts on its
side effects, and passes its returned state to child tests. Test output reads like a
domain narrative (Given → When → Then) — if the output doesn't read naturally, the test
is wrong.

## Read the version-matched docs first

The full guides ship inside the testscribe package itself. Always read them from the
version this project actually uses:

1. Resolve the version: `grep -A1 '^name = "testscribe"$' Cargo.lock`
2. Find the package sources: `ls -d ~/.cargo/registry/src/*/testscribe-<version>`
   (if missing, run `cargo fetch` once)
3. Read, in this order:
   - `docs/llms.md` — the agent guide: workflow, `then!` decision tree, good/bad patterns
   - `docs/foundations.md` — the test-tree philosophy and the SHAPE loop
   - `docs/guidelines.md` — naming and assertion rules with examples
   - `tests/*.rs` — complete runnable examples for every feature (environments, cloning,
     custom checks, parameterized tests, custom runners)
   - `docs/advanced_techniques.md` — only when environments, state cloning, or custom
     checks are involved
   - `docs/does_it_fit.md` — when deciding whether testscribe fits a use case at all:
     linear stories and test matrices (pure functions, time-dependent behavior,
     multi-actor scenarios) vs. cases it doesn't fit (concurrency, property tests, benchmarks)
   - `docs/testing_checklist.md` — apply after writing each test

Do not write tests from memory of this skill alone; the docs above are canonical for the
installed version.

## Workflow

1. **See the current tree**: run `cargo test -- --nocapture` and read the output.
   Identify existing states and which business actions are not covered yet.
2. **Pick the parent**: hook into existing state from a parent test (`Given<...>`).
   Create a root (`standalone`) test only when no earlier business step exists.
3. **Write one business action** that produces observable side effects. Assert what
   changed, what must NOT have changed, and probe actions that just became impossible.
4. **Re-run and read the output line by line** — every `Then` line must make sense to
   someone who knows the domain but not the code.
5. **Go through `docs/testing_checklist.md`** before moving on.

## Stable essentials

- Test names sound like events: `payment_attempt_failed`, not `test_payment`.
- `then!(variable).eq(..)` / `then!(expr => alias).eq(..)` /
  `then!("statement").run(|| ..)` / `then!("statement").params(list).run(|item| ..)`
- Async parents require async children.
- Custom checks consume `self`: name them `rejected_as_*` / `has_*`, never `is_*`/`to_*`
  (clippy `wrong_self_convention`).
- Separate **test state** (returned by tests) from **environment state** (`Env<E>`,
  infrastructure); clone expensive setups via `cloneable`/`cloneable_async` instead of
  letting parents re-run.

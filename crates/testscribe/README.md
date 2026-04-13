# testscribe

A Rust testing framework for stateful systems. Tests build on each other's state, forming a **test tree**: each test performs one business action, asserts on its side effects, and passes its resulting state to child tests. The output reads like a domain narrative.

## 2-Minute Quickstart

### 1) Add dependency

```bash
cargo add testscribe --dev
```

### 2) Create `tests/demo.rs`

```rust
use std::collections::HashMap;

use testscribe::prelude::*;

type Cache = HashMap<String, i32>;

#[testscribe(standalone)]
fn value_4_is_added_to_cache() -> Cache {
    let mut cache = Cache::new();
    cache.insert("key".to_owned(), 4);
    then!(cache["key"] => it).eq(4);
    cache
}

#[testscribe]
fn key_is_deleted(mut state: Given<Value4IsAddedToCache>) {
    state.remove("key");
    let value = state.get("key");
    then!(value).eq(None);
}

#[testscribe]
fn value_is_replaced_to_5(mut state: Given<Value4IsAddedToCache>) {
    let previous_value = state.insert("key".to_owned(), 5).unwrap();
    then!(state["key"] => it).eq(5);
    then!(previous_value).eq(4);
}
```

### 3) Run tests

```bash
cargo test -- --nocapture
```

Example output:

```text
 | 0.013ms|Given value 4 is added to cache
 |       -|  Then it is equal to 4
 | 3.581μs|  When key is deleted
 |       -|    Then value is equal to None
 | 2.950μs|  When value is replaced to 5
 |       -|    Then it is equal to 5
 |       -|    And previous_value is equal to 4
```

Instead of isolated test results, you get a readable scenario: **Given → When → Then**

## Key pieces

- `#[testscribe(standalone)]` marks a **root test**; running it via `cargo test` runs its whole subtree. Every test generates a PascalCase type (`value_4_is_added_to_cache` → `Value4IsAddedToCache`) that child tests reference via the `Given` argument to receive the parent's state.
- The `then!` macro records checks that show up in the output:
  - `then!(variable).eq(...)` — verify a value, named after the variable
  - `then!(expression => alias).eq(...)` — verify an expression under a readable alias
  - `then!("statement").run(|| ...)` — run a closure under a narrative statement
  - `then!("statement").params(list).run(|item| ...)` — table-style checks
- `Env<E>` arguments provide infrastructure state (database pools, mocks) separately from test state; `Param<P>` arguments run a test once per parameter.
- Async tests, custom domain-specific checks, state cloning (to avoid re-running expensive parents) and custom test runners are all supported.

## Documentation

`testscribe` introduces conventions and a specific way of thinking. Taking several minutes to read the foundations will save you hours later.

- [Foundations](https://github.com/fraillt/testscribe/blob/main/crates/testscribe/docs/foundations.md) — the test tree philosophy and the SHAPE loop; start here
- [Guidelines](https://github.com/fraillt/testscribe/blob/main/crates/testscribe/docs/guidelines.md) — how to prevent common mistakes
- [Advanced techniques](https://github.com/fraillt/testscribe/blob/main/crates/testscribe/docs/advanced_techniques.md) — environments, custom checks, state cloning, custom runners
- [Does your use case fit testscribe?](https://github.com/fraillt/testscribe/blob/main/crates/testscribe/docs/does_it_fit.md) — when testscribe fits (linear stories, test matrices) and when it doesn't (concurrency, property tests, benchmarks)
- [Testing checklist](https://github.com/fraillt/testscribe/blob/main/crates/testscribe/docs/testing_checklist.md) — quick checklist to apply after writing each test
- [API reference](https://docs.rs/testscribe) on docs.rs
- [Features showcase](https://github.com/fraillt/testscribe/tree/main/crates/testscribe/tests) — compact syntax examples for every feature

These guides are shipped inside the published crate (`docs/` folder), so they are available offline wherever the crate source is — including your local cargo registry.

## For AI Agents

Start with [llms.md](https://github.com/fraillt/testscribe/blob/main/crates/testscribe/docs/llms.md) — it defines how agents should write idiomatic testscribe tests. It ships inside the package together with the other guides, so agents can read everything offline from the local cargo registry.

To make agents pick this up automatically, add this to your project's `AGENTS.md` (or `CLAUDE.md`):

```text
This project tests with the `testscribe` crate. Before writing or changing tests, read
`docs/llms.md` from the testscribe package sources in the local cargo registry:
`~/.cargo/registry/src/*/testscribe-<version>/` (match the version in Cargo.lock).
```

Claude Code users can install the testscribe skill instead, which triggers automatically
whenever tests are written in a project that depends on testscribe:

```text
/plugin marketplace add fraillt/testscribe
/plugin install testscribe@testscribe
```

## Cargo features

- `standalone` *(enabled by default)* — run test trees through the standard test runner via
  `#[testscribe(standalone)]`; also provides utilities for building custom runners.
- `detached` — experimental remote-controlled test runner.

## License

Licensed under the [MIT license](https://github.com/fraillt/testscribe/blob/main/LICENSE).

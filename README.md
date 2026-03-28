# testscribe

**testscribe** is a Rust testing framework for stateful systems.

It turns tests into readable domain scenarios —
not just a list of `... ok` lines.

Tests don’t verify functions.
They **shape behavior**.

---

## The SHAPE Loop

testscribe follows a simple workflow:

- **Select** a single business action
- **Hook** into existing state
- **Assert** in natural language
- **Probe** what changed
- **Expand** the test tree

This produces structured, event-driven test trees where each node represents a real domain side effect.

→ Read the full philosophy in [`docs/foundations.md`](./docs/foundations.md)

---

## 2-Minute Quickstart

### 1) Add dependency

```bash
cargo add testscribe --dev
```

### 2) Create `tests/demo.rs`

```rust
use std::collections::HashMap;

use testscribe::report::basic::CheckEq;
use testscribe::test_args::Given;
use testscribe::testscribe;

type Cache = HashMap<String, i32>;

#[testscribe(standalone)]
#[test]
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

```bash
 | 0.013ms|Given value 4 is added to cache
 |       -|  Then it is equal to 4
 | 3.581μs|  When key is deleted
 |       -|    Then value is equal to None
 | 2.950μs|  When value is replaced to 5
 |       -|    Then it is equal to 5
 |       -|    And previous_value is equal to 4
```
Instead of isolated test results, you get a readable scenario: **Given → When → Then**

---

## Why testscribe?

- Models real domain events
- Encourages side-effect driven testing
- Produces readable output
- Naturally builds test trees from behavior
- Makes tests understandable beyond developers

---

## Where to start

- Read the [motivation](./docs/motivation.md)
- Study the core ideas in [foundations](./docs/foundations.md)
- Explore [advanced techniques](./docs/advanced_techniques.md)
- Review the [guidelines](./docs/guidelines.md)

testscribe introduces conventions and a specific way of thinking.
Taking several minutes to read the foundations will save you hours later.

Happy coding!

## For AI Agents

Start with [llms.md](docs/llms.md).

# testscribe

**testscribe** is a Rust testing framework for stateful systems.

It turns tests into readable domain scenarios — not just a list of `... ok` lines.

Tests don’t verify functions.
They **shape behavior**.

---

[![CI](https://github.com/fraillt/testscribe/actions/workflows/ci.yml/badge.svg)](https://github.com/fraillt/testscribe/actions/workflows/ci.yml)

## Why testscribe?

Traditional test output tells you *that* something passed, not *what* your system does:

```text
test cancel_order_restores_inventory ... ok
test partial_refund_updates_order_state ... ok
test happy_path_checkout_to_delivered ... ok
```

How many steps are there from checkout to delivery? Are there assertions at every step,
or only on the final state? You can't tell without reading the test code.

testscribe output reads like a domain narrative instead:

```text
 | 0.013ms|Given value 4 is added to cache
 |       -|  Then it is equal to 4
 | 3.581μs|  When key is deleted
 |       -|    Then value is equal to None
 | 2.950μs|  When value is replaced to 5
 |       -|    Then it is equal to 5
 |       -|    And previous_value is equal to 4
```

- Models real domain events
- Encourages side-effect driven testing
- Builds test trees naturally from behavior — each test reuses its parent's state, so most test code actually tests something instead of setting the stage
- Makes tests understandable beyond developers

→ Read the full story in [motivation](./docs/motivation.md)

---

## The SHAPE Loop

testscribe follows a simple workflow:

- **Select** a single business action
- **Hook** into existing state
- **Assert** in natural language
- **Probe** what changed
- **Expand** the test tree

This produces structured, event-driven test trees where each node represents a real domain side effect.

→ Read the full philosophy in [foundations](./crates/testscribe/docs/foundations.md)

---

## Getting started

Head over to the [`testscribe` crate](./crates/testscribe/README.md) for a 2-minute quickstart
and the full user documentation, or explore by example:

- [features showcase](./crates/testscribe/tests/README.md) — compact syntax examples for every feature
- [checkout domain](./examples/checkout-domain/README.md) — a realistic end-to-end example (Postgres via testcontainers)

## Repository structure

| Path | Description |
|------|-------------|
| [crates/testscribe](./crates/testscribe/) | The main crate users depend on; re-exports everything below and ships the user documentation |
| [crates/core](./crates/core/) | Test tree structure, execution logic and assertion/reporting types |
| [crates/proc-macros](./crates/proc-macros/) | The `#[testscribe]` attribute and `ParamDisplay` derive |
| [crates/standalone](./crates/standalone/) | Default test runner, integrates with `cargo test`; also building blocks for custom runners |
| [crates/detached](./crates/detached/) | Experimental remote-controlled test runner |
| [examples/](./examples/) | Example projects (not published) |

## For AI Agents

Point your agent at [llms.md](./crates/testscribe/docs/llms.md) — it defines how agents should write idiomatic testscribe tests. These docs are also shipped inside the published crate, so agents can discover them offline.

This repository is also a Claude Code plugin marketplace hosting the [testscribe skill](./skills/testscribe/SKILL.md), which triggers automatically whenever tests are written in a project that depends on testscribe:

```text
/plugin marketplace add fraillt/testscribe
/plugin install testscribe@testscribe
```

See the [`testscribe` crate README](./crates/testscribe/README.md#for-ai-agents) for a tool-agnostic `AGENTS.md` snippet as well.

## Contributing

Issues and pull requests are welcome. If you're proposing a larger change, please open an issue first to discuss the direction.

## License

Licensed under the [MIT license](./LICENSE).

## 💖 Sponsor Me?

If you find this project useful or interesting, or just want to say thanks, you can buy me a coffee!
Your support keeps me motivated to maintaining and improving this project.

[**Thank you!**](https://github.com/sponsors/fraillt)

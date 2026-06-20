# testscribe: LLM/Agent Guide

This file defines how AI agents should write tests with `testscribe`.

## Workflow

Follow this loop for every test you write:

1. **See the current tree**: run `cargo test -- --nocapture` and read the output.
   Identify existing states and which business actions are not covered yet.
2. **Pick the parent**: hook into existing state with `Given<ParentNameInPascalCase>`.
   Create a root test (`standalone`) only when no earlier business step exists.
3. **Write one business action** that produces observable side effects. Assert what
   changed, what must NOT have changed, and probe actions that just became impossible.
4. **Re-run and read the output line by line** — every `Then` line must make sense to
   someone who knows the domain but not the code.
5. **Apply the [testing checklist](testing_checklist.md)** before moving on.

## Core information

MUST be read to idomatically write `testscribe` tests.

- Understand testscribe [foundations](foundations.md), especially the SHAPE loop.
  - S - define new test function
  - H - find the right parent test to reuse it's state
  - A - assert on occured side effects
  - P - assert on actions that became impossible
  - E - explore what new actions become possible, and go to "S" step.
- Understand the **syntax** for particular library [feature](../tests/README.md)
  - Understand how written code affects generated test outcome
  - IMPORTANT: reading test output line-by-line should feel natural.
- Follow [guidelines](guidelines.md), to write idiomatic test code.
- When a test does not obviously fit the tree model (pure functions, time-dependent
  behavior, multi-actor scenarios), check [does your use case fit testscribe?](does_it_fit.md)
  before falling back to plain `#[test]` functions — it usually still fits as a linear
  story or a test matrix. That doc also covers what genuinely doesn't fit (concurrency,
  property tests, benchmarks).
- After you write a test make sure it passes the [testing checklist](testing_checklist.md)

## Good patterns, anti-patterns

It is intentionally practical: good patterns, anti-patterns.

### Good test function properties

- Performs domain action that has observable side effects
  - Most of the time action have positive outcome
  - but negative outcome with side effects exists as well,
  e.g. user entering invalid password 3 times is blocked.
- Checks all the effects that happened
- Checks effects that should not happen (e.g. doesn't have side effects)
  - new functionality: e.g. user has registered, but still cannot login until he is activated
  - existing functionality: e.g. registering same user twice returns "already registered" error.

### Bad test function properties

- Performs action that is rejected without any visible side effects
- Checks only on effects that happened, but doesn't check what should not happen
- Doesn't perform any action at all, but simply assert on some state
- Assertion text is semantically different from what it is asserting on

### Quick decision tree to pick the right `then!` form.

1. Do you verify one concrete value with a clear variable name?
	- Use: `then!(variable).extension(...)`
2. Do you verify an expression and want readable output label?
	- Use: `then!(expression => alias).extension(...)` (expression form requires alias)
3. Do you want one narrative check that runs code and may fail/panic?
	- Use: `then!("statement").run(...)` or `run_async(...)`
4. Do you verify the same condition over many items?
	- Use: `then!("statement").params(list).run(...)`
5. Is built-in output too generic (for example boolean `is_err`) and domain wording matters?
	- Use: a project-local custom check trait on `VerifyValue<'_, T>`.

#### More elaborate examples

#### 1) Verify value (variable name)

Syntax:
- `then!(variable_name).extension(...)`

Example:
- `then!(available_stock).eq(3);`

When to use:
- You already have a clearly named variable and want readable output.

Common misuse:
- Asserting on unclear boolean variables (`is_ok == true`) when a semantic check is possible.

#### 2) Verify value (expression with alias)

Syntax:
- `then!(expression => alias_name).extension(...)`

Example:
- `then!(user.age + 1 => next_year_age).eq(31);`
- `then!(format!("{} {}", user.name, user.surname) => full_name).eq("Charlie Smith");`

When to use:
- You have result object and want to nicely display some of it's properties
- The expression is useful, but its raw text would be noisy in output.

Common misuse:
- Using aliases that are too generic (`value`, `result`, `tmp`).
- Using complicated expressions that involve service calls inside of it.
- Creating aliases for already clear variables (adds noise without benefit).
- Writing expression form without alias (does not compile), for example `then!(user.age).eq(31);`.

#### 3) Verify statement (group related checks)

Syntax:
- `then!("statement").run(|| { ... })`
- `then!("statement").run_async(async || { ... })`

Example:
- `then!("payment is accepted").run(|| service.is_payment_accepted(order_id));`

Behavior:
- The check passes on success.
- The check fails if the closure returns an error outcome, panics, or returns `false`.

When to use:
- Multiple tiny checks belong under one narrative statement.

Common misuse:
- Putting unrelated assertions into one statement block.

#### 4) Verify params (table-style checks)

Syntax:
- `then!("statement").params(list).run(|item| { ... })`
- `then!("statement").params(list).run_async(async |item| { ... })`

Example:
- `then!("order has these items").params(expected_items).run(|item| actual.contains(&item));`

When to use:
- Repeated checks over a list where row-by-row output is valuable.

Common misuse:
- Parameter tables for one-off checks (use normal value assertion instead).

#### 5) Project-local custom checks

Pattern:
- Implement a trait for `VerifyValue<'_, T>` in test code.
- Use `VerifyValueExposed::new(self)` and `reporter.set_outcome(...)`.

Multiple facts per check:
- `set_outcome` takes `&self`, so one check may call it several times — each call is its own
  `Then ...` line. Use this when a domain check naturally verifies several related facts (e.g.
  `transfers(from, to, amount)` reporting debit and credit with amount as two lines).

Example:
- `then!(checkout_result).rejected_for_insufficient_stock(product_id);`

When to use:
- Domain-specific assertions that are clearer than generic `eq(true)` or `is_err()`.

Naming:
- Avoid `is_*` and `to_*` prefixes: custom checks consume `self` by value, which triggers
  clippy's `wrong_self_convention` lint for those prefixes (a problem under `-D warnings`).
- Prefer outcome/event wording, which also reads better in test output:
  `rejected_as_...`, `has_...`, `sorted_ascending`, `matches_...`.

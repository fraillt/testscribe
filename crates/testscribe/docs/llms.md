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

## State between parent and child

A test receives what its parent returned through `Given<Parent>`, and passes on what it returns.
Pick the form by what the test does with the state:

- **Only reads it**: `cart: Given<CartCreated>`, then `cart.items` or `*cart` (`Given` derefs to
  the state). No `mut`: a `mut` that is not needed is an `unused_mut` warning.
- **Changes it and passes it on**: `mut cart: Given<CartCreated>`, change it through `cart`,
  and return `cart.into_inner()`.
- **Passes on a different type**: take the state by value, either by destructuring in the
  argument, `Given(cart): Given<CartCreated>`, or with `cart.into_inner()` in the body, and
  build the new state from it:

  ```rust
  #[testscribe]
  fn cart_checked_out(Given(cart): Given<CartCreated>) -> Order {
      let order = checkout(cart);
      then!(order.status => order_status).eq(OrderStatus::Placed);
      order
  }
  ```

- A test that returns nothing passes `()` to its children.

Reference: [basic_tests_tree.rs](../tests/basic_tests_tree.rs)

## No action to test: pure functions

A conversion, a parser or a validation rule has inputs and outputs and no side effects, so there
is no action and no tree. Write one standalone test with a table of cases; each case becomes a
row in the output:

```rust
#[derive(Clone, ParamDisplay)]
struct NameCase {
    name: &'static str,
    accepted: bool,
}

#[testscribe(standalone)]
fn user_name_validation_rules_defined() {
    then!("user name is accepted or rejected by validation rules")
        .params([
            NameCase { name: "alice", accepted: true },
            NameCase { name: "al", accepted: false },
        ])
        .run(|case| validate_user_name(case.name).is_ok() == case.accepted);
}
```

More on this and on other cases that do not obviously fit: [does it fit?](does_it_fit.md)

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
5. Do you verify what a value is (for example which enum variant), not what it equals?
	- Use: `then!(variable).is("description", |value| matches!(value, ...))` or `is_not(...)`
6. Is built-in output too generic (for example boolean `is_err`) and domain wording matters?
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

#### 5) Verify what a value is (predicate)

Syntax:
- `then!(variable).is("description", |value| { ... })`
- `then!(variable).is_not("description", |value| { ... })`

Example:
- `then!(step).is("a rollback", |step| matches!(step, Step::RolledBack { .. }));`

Behavior:
- Output reads `Then step is a rollback` (`is not` for `is_not`); the description completes the sentence.
- The check fails if the predicate returns `false` (`true` for `is_not`) or panics; the actual value is shown.

When to use:
- Enum variants with payloads, or any property a pattern or a short predicate expresses.
- Instead of `then!(matches!(..) => alias).eq(true)` and instead of `then!("...").run(|| matches!(..))`.

Common misuse:
- A description that does not match the predicate.
- The same predicate repeated across many tests (write a custom check instead).

#### 6) Project-local custom checks

Pattern:
- Implement a trait for `VerifyValue<'_, T>` in test code.
- Use `VerifyValueExposed::new(self)` and `reporter.set_outcome(...)`.

Example:
- `then!(checkout_result).rejected_for_insufficient_stock(product_id);`

When to use:
- Domain-specific assertions that are clearer than generic `eq(true)` or `is_err()`.
- Structs whose `Debug` would fill the output when compared with `eq`: define a `Told` trait
  (`fn told(&self) -> String`) and one `reads` check for every `T: Told`, once per project, then
  `then!(transfers[0] => deposit).reads("Confirmed Incoming 50.000000 in block 101");`
  (see "Telling domain values" in `advanced_techniques.md` and `tests/custom_checks.rs`).
  Do not write a formatting helper per test file.

Naming:
- Avoid `is_*` and `to_*` prefixes: custom checks consume `self` by value, which triggers
  clippy's `wrong_self_convention` lint for those prefixes (a problem under `-D warnings`).
- Prefer outcome/event wording, which also reads better in test output:
  `rejected_as_...`, `has_...`, `sorted_ascending`, `matches_...`.

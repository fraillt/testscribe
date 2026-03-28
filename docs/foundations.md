## Overview

`testscribe` excels when testing stateful code with multiple state transitions and branches. For simple functions with straightforward input/output validation, Rust's standard `libtest` framework works well. However, `testscribe` offers more detailed and readable test outcomes that many developers find valuable even in simpler scenarios.

This document walks through the philosophy and practical application of the `testscribe` framework, starting with basic library features and ending with advanced techniques for writing tests efficiently.

## Traditional tests don't tell the full story

Traditionally, it's common to write a single test for the "happy path" and assert only on the final state, then add a few more tests that cover failure cases (see [checkout service tests](../examples/checkout-domain/tests/traditional_tests.rs)). When you run these tests, you might see output like:
```text
test cancel_order_restores_inventory ... ok
test partial_refund_updates_order_state ... ok
test happy_path_checkout_to_delivered ... ok
test payment_failure_then_retry_success ... ok
```
While technically we see that there are 4 tests, in practice it's not clear how much they cover.
If you already know the domain, you can at least understand that the happy path works and that some edge cases are covered, but some things can only be answered by analyzing the test code itself, such as:
- how many steps there are from checkout to delivery?
- do the tests assert only on the final state, or are there assertions at every step?
- how the customer cart works and how it interacts with product stock
- is there a product stock check at all? does it happen when adding to cart, or at checkout?
- what are state transitions and when they happen?
- do we have some sort of event history?
- etc...

---

## `testscribe` - A Test Tree Philosophy

The essence of `testscribe` is a concept called a **test tree**.

You build a tree where each node represents a **real business event** - not just a function call.
Each node:

- Tests a SINGLE business/domain action that produces a side effect.
- Reuses its parent node’s state as the starting point (except for root nodes).

This structure keeps tests aligned with real domain behavior instead of implementation details.

---

### Side Effects Define the Tree

One of the most common mistakes in testing is treating every function call as a test case.

In `testscribe`, a node represents something more specific:

> A new observable side effect in the domain.

In programming terms, a side effect is any observable change beyond returning a value.

In domain tests, this typically means:

- Persisted state changed (insert/update/delete, status transitions)
- Domain artifacts were recorded (notifications, audit logs, events)
- An externally visible integration action occurred

These are **not** side effects by themselves:

- Read-only queries (`get`, `list`, `find`)
- Rejected actions that returned an error but changed nothing
- In-memory calculations that are not persisted or observable

#### Practical Rule

1. If an action produced a new observable change - it deserves its own test node.
2. If an action was rejected and nothing changed - assert it near the closest side-effecting node.
3. If your setup helper performs meaningful business actions - promote it to a proper test node.

---

### Why This Matters

- Side effects are **evidence**. They prove behavior changed.
- Each node represents a real domain event - which makes test output readable.
- Side effects naturally generate the next possible actions.
- The tree grows organically:
  **event → next possible events**
- You avoid brittle, disconnected tests that only verify rejections.

When done correctly, your test output should be readable not only to you - but also to your PM.

---

## The SHAPE Loop

To build your test tree correctly, follow the SHAPE loop:

### S — Select a Business Action

That's your test function (node in a test tree).

Choose a SINGLE meaningful domain action:
- not a Rust function.
- not a database operation.
- a business action.

If it’s hard to explain why the test matters, it’s probably too narrow.

Example:

You don’t care whether the checkout service successfully connected to the database.
You care that:

- Stock is reduced.
- Checkout fails when stock is insufficient.

That’s business behavior.

---

## H — Hook into Existing State

Always build on existing state (previously defined test).

Every node (except the root) should depend on its parent’s returned state.

If there’s no state to build on:

- Either this is a root node,
- Or you skipped testing an earlier business step.

The tree should reflect real-world progression.

---

## A — Assert in Natural Language

Express expectations using `then!`.

Assertions should read like business statements:

- “Stock was reduced by 1.”
- “Checkout was rejected due to insufficient stock.”
- “Payment was recorded.”

Customize your assertion layer to improve ergonomics and output clarity.

The goal: test output that reads like a domain narrative.

---

## P — Probe What Changed

After a side effect occurs, ask:

- What actions are now impossible?
- What actions are now enabled?

For example:

- A closed ticket should not accept new comments.
- A paid order can now be shipped.

Assert the invalid paths in the same test.
Explore the new valid paths in the new tests.

---

## E — Expand the Tree

While domain flow is not complete, every side effect creates new possible branches.

Each new valid action becomes a child node.

This is how the tree grows:
- Event → Next possible event → Next possible event

Natural. Structured. Domain-driven.

---

## That’s It

Follow the SHAPE loop:

- Select
- Hook
- Assert
- Probe
- Expand

If you do this consistently, you’ll get:

- Clear test structure
- Domain-aligned output
- Naturally evolving coverage
- High signal, low noise tests

## The elephant in the room

If you're an experienced developer, you're probably thinking:
- but it's just another BDD (GWT or AAA or 3A) testing framework, right?

Having experience with these frameworks might help you get into the right mindset faster, but `testscribe` differs in a few areas:
- it's purely developer-centric - other frameworks (and testing philosophies) often lean too much on the business side and don't address development ergonomics enough. Since each test in `testscribe` depends on existing state, you'll forget what it's like to maintain multiple helper functions that are used to set up the initial state for each test. Most of the test code will actually test something, instead of setting the stage for the test.
- building a test tree is not the same as writing a feature/scenario. Instead of focusing on how to implement one specific test, you mostly focus on how to build a complete test tree. E.g. where does my test fit in the existing tree, and what new branches (tests) does it create?

## Next steps

- [Checkout service](../examples/checkout-domain/README.md) - a more realistic end-to-end example.
- [Advanced techniques](./advanced_techniques.md) - useful next step to be more efficient.
- [Guidelines](./guidelines.md) - explanation on how prevent common mistakes
- [Testing checklist](./testing_checklist.md) - quick practical checklist you can apply after writing each test.

## A note on real-world setups

Real-world test suites often interact with external systems (databases, file systems, message queues, etc.). Two advanced techniques become especially relevant:

- **Cloning external state**: when optimizing execution speed, your `Clone` / `CloneAsync` implementation may need to duplicate external resources. For example, PostgreSQL supports `CREATE DATABASE ... WITH TEMPLATE ...` to snapshot a database for each branch. See [Advanced Techniques — State cloning](./advanced_techniques.md) for patterns and trade-offs.
- **Custom checks**: domain-specific assertions (e.g. verifying order status transitions, checking notification history) benefit from [custom checks](./advanced_techniques.md) that improve both code ergonomics and test output readability.

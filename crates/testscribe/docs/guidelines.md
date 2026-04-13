## 1. Test names should sound like events

### Bad:

```
fn ticket_has_comments_support(...)
```
"has" and "support" sound like capability, not an action.

### Good:

Reflect what side effect happened inside the test function. It should read like an event.
```
fn ticket_comment_added(...)
fn comment_added_to_ticket(...)
```

## 2. Avoid checks that only assert true/false for non-boolean expressions

### Exception:

Unless the value you assert on is boolean. E.g. `then!(is_user_active).eq(true)`

### Bad:

```
then!(matches!(err, DomainError::InvalidState(_)) => error_is_invalid_state).eq(true);
```
It's hard and unnatural to read this in code and in test output.

### Good:

If you assert this way in only a few places, you can convert it to a string and assert on that instead.
```
then!(err.to_string() => insert_error).contains("ticket already closed");
```

## 3. Check messages and assertion labels should describe intent directly in test output

When someone reads test output, they should understand intent without opening code or comparing hidden values.
This applies both to `VerifyStatement` (e.g. `then!("...").run(...)`) and to named value checks (e.g. `then!(expr => alias).eq(...)`).

### Bad:

```
then!("user exists").run(|| {
    env.svc.create_user("alice@example.com", "Alice2").await.is_err()
}).await;
```
Test outcome would show `Then user exists`, but code was actually trying to create another user instead of checking if user exists.

```
let reloaded_user_id = env.svc.create_or_load_user(customer_id).await.unwrap();
then!(reloaded_user_id).eq(cart_id);
```
Output like `Then reloaded cart id is equal ...` does not clearly communicate business intent.

### Good:

```
then!("creating same user returns an error").run(|| {
    env.svc.create_user("alice@example.com", "Alice2").await.is_err()
}).await;
```
Here test output would align with what code is doing.

```
then!("creating user again returns existing user").run_async(async || {
    let reloaded_user_id = env.svc.create_or_load_user(customer_id).await.unwrap();
    reloaded_user_id == user_id
}).await;
```
This message tells the reader exactly what behavior is being checked.

## 4. Implement custom checks for common checks/assertions

### Bad:

```
then!(matches!(err, DomainError::InvalidState(_)) => error_is_invalid_state).eq(true);
```
Hard to write and hard to read.

### Good:

If this is a common theme in many tests, consider implementing [custom checks](../tests/custom_checks.rs).
This not only makes code easier to write, but also makes test output exactly what you want it to be.
```
then!(insert_error).has_domain_error(DOMAIN_ERR_INVALID_STATE)
```

## 5. Verify claimed behavior when APIs are write-focused

Comments are not assertions. If a test claims a behavior, it should verify that behavior explicitly.

### Bad:

```
// Add same item again – quantity should stack (upsert)
env.svc.add_item(state.cart_id, item, 1).await.unwrap();
```

The test claims quantity stacking, but no check confirms the final quantity.

### Good:

After the action, assert an observable outcome that proves the claim (for example, resulting quantity, totals, statuses, notifications, or domain errors).

If write APIs do not expose enough state, use this order:

1. **Public behavior first**: verify through existing public/read methods and domain-visible outcomes.
2. **Test-side inspector next**: add test-only read helpers near tests.
3. **Direct infra query last**: query DB/files/queues directly only when needed, and keep those checks centralized.

This keeps production APIs focused while still allowing precise, trustworthy behavior checks.

## 6. Never perform side effects inside `then!`

A check observes state; it must not change it. Calling a side-effecting action (one that inserts/updates/deletes, transitions status, emits notifications, etc.) inside a `then!` expression is forbidden: it hides a real business action inside something that reads like an assertion, so the test tree can no longer separate "what happened" from "what we verified".

### Bad:

```
// a state transition is hidden inside an assertion
then!(env.svc.checkout_cart(cart_id).await.unwrap() => order_id).ne(Uuid::nil());
```

### Good:

Perform the action as the node's business step, then assert on its observable result:

```
let order_id = env.svc.checkout_cart(cart_id).await.unwrap();
then!(env.svc.order_status(order_id).await.unwrap() => order_status).eq("placed".to_string());
```

> Read-only calls inside `then!` (for example reading a status just to assert on it, as above) are **not** forbidden. Whether to inline such a read or bind a `let` first is a readability choice left to you.

## 7. Build the trunk on the happy path; branch exceptional cases off it

A test tree has one longest path — its trunk. Make that the happy path: each node continues from the **successful** outcome of the previous business step. Exceptional outcomes — a declined payment, insufficient stock, a cancellation, a refund — branch off the trunk as their own nodes and stay shallow (often leaves).

### Bad:

The mainline grows *through* a failure and its recovery:

```
cart_checked_out → payment_attempt_failed → payment_retried_with_success → order_shipped → order_delivered
```

- The only path to a paid order goes through a decline, so first-try capture is never tested.
- Every downstream node inherits the failure's residue (a stray `failed` payment row, an extra `payment_failed` notification), so `["failed", "captured", "refunded"]` shows up where the normal case is `["captured", "refunded"]`.

### Good:

Capture-on-first-try is the trunk; decline-then-retry is a sibling branch that proves recovery and ends there:

```
cart_checked_out
├── payment_captured → order_shipped → order_delivered → …   (trunk: reads start-to-finish)
└── payment_attempt_failed → payment_retried_with_success     (self-contained recovery story)
```

Rule of thumb: when a step has both a success and a failure outcome, continue the trunk from success and make failure a sibling.

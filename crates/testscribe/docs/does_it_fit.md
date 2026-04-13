# Does your use case fit testscribe?

The rest of the documentation speaks in the voice of the ideal `testscribe` use case: a stateful domain, business actions with side effects, a test tree grown through the SHAPE loop. Sooner or later, though, you'll reach for `testscribe` on a test that doesn't obviously look like that, and you'll catch yourself asking:

> Does my use case actually fit the `testscribe` philosophy?

There are only two answers: **it fits**, or **it doesn't**. This page is about telling them apart — and about the rough edges where the honest answer is "it fits, just not in the shape you first expected."

The rule of thumb is short. **`testscribe` fits whenever your test is one of these two shapes:**

- **A clear, linear story** — a sequence of events and state transitions you want to read top to bottom.
- **A finite set of test cases** — a test matrix you can enumerate and lay out row by row.

**`testscribe` doesn't fit when it's neither** — when the execution model isn't linear (concurrency, property/fuzz testing) or when there's nothing worth describing about the outcome (benchmarks). Each is covered below.

When your test *is* one of the two shapes, keep it in `testscribe` even when it's a "lesser" use that gives up state reuse or the SHAPE loop — **keeping the whole suite in one framework with one output format is valuable in itself.** Once part of your suite reads as a narrative, flat `test foo ... ok` lines sprinkled in between hide information the rest of the suite makes visible.

---

# It fits — a clear, linear story

This is the home turf: a stateful domain where each node is a business event, each child builds on its parent's state, and the output reads as a narrative. Everything else in the documentation is about this shape, so there's nothing new to say here — except that two situations *feel* like they don't fit and actually do. They only look wrong because the event is hidden, not because the story isn't there. Reframe, and they slot into the tree like any other node.

## Time-dependent behavior

**The situation:**
Carts expire after 30 days, reports run nightly, reservations time out. Nobody *performs* "time passing", so it doesn't feel like a business action — and you can't `sleep(30 days)` in a test.

**The testscribe way:**
Time passing **is** an event, and it fits the tree without any special treatment. Two ingredients:

1. Put a controllable clock in the [environment](../tests/environment.rs) (infrastructure state), and make the system under test read time from it.
2. Write the node as the event that time triggered, with its observable side effects:

```rust
/// Time passing is the event; the expired cart is its side effect.
#[testscribe]
async fn thirty_days_passed_and_idle_cart_expired(
    state: Given<ItemsAddedToCart>,
    env: Env<ShopEnv>,
) {
    env.clock.advance(days(30));
    env.service.run_scheduled_jobs().await.expect("run scheduled jobs");

    let status = cart_status(&env.service, state.cart).await;
    then!(status => cart_status).eq("expired");

    // an expired cart no longer accepts items
    let add_after_expiry = env
        .service
        .add_item_to_cart(state.cart, state.shop.keyboard, 1)
        .await;
    then!(add_after_expiry).rejected_as_invalid_state("can only add items to active cart");
}
```

The full SHAPE loop applies: the expiry is a side effect, it disables actions (adding items), and it may enable new ones (an expired cart could be revived — a child test). The only unusual part is *who* the actor is: the clock instead of a user.

**What still applies:**

- The advanced clock state (current time offset) is **environment state**, not test state — children of this node inherit the advanced clock through the environment.
- If different branches need different times, advancing the clock inside each branch keeps the tree honest: each node states how much time passed before its action.

## Independent actions and the single-parent rule

**The situation:**
Two actions where neither depends on the other — register a customer *and* stock a product; create the admin *and* create the shopper. Nothing says which comes first, yet a node has exactly **one** parent: the tree can't branch two independent lineages and rejoin them. So you must put them in *some* order.

The good news: when the actions are genuinely independent, the order you pick can't change the outcome — **linearizing is almost free.** The only cost is that the reader sees a sequence the domain didn't require. How you pay it depends on how much the action deserves to be *seen* at all:

**1. Nothing to assert → fold it into the starting state.**
If the action is pure fixture — it sets up the world but has no side effect you'd probe (a product simply existing, an admin simply existing) — it doesn't need a node. Seed it in the root node with the other initial data, or, if it's infrastructural rather than business data, in the [environment](./advanced_techniques.md#1-use-environments-to-model-state-ownership-explicitly). The story then starts where it gets interesting. Be honest about the bar, though: the moment you *do* want to assert something — "stock starts at 10" — the action has earned a node, and burying it in the environment would bury a real check.

**2. A couple of checks, but no story of its own → merge into one node.**
If the action is worth verifying but too slight to anchor its own subtree, perform both actions in a single node. You still choose an explicit order in the code, but the reader sees one logical step — one `When …` line instead of two near-empty ones.

**3. A genuine business event → give it its own node and pick an order.**
When the action carries its own weight — side effects, probes, children — make it a node and choose a position in the lineage. The single-parent rule isn't fighting you here; real systems order their events too. Let the node name (and a one-line comment) record that the order was a free choice, so nobody mistakes it for a causal dependency.

```rust
/// The admin steps in; the customer's shopping is frozen mid-flight.
#[testscribe]
async fn customer_suspended_by_admin(state: Given<ItemsAddedToCart>, env: Env<ShopEnv>) {
    let svc = &env.service;
    svc.suspend_customer(state.shop.customer, "fraud review")
        .await
        .expect("suspend customer");

    let status = svc.customer_status(state.shop.customer).await.expect("query status");
    then!(status => customer_status).eq("suspended");

    // a suspended customer cannot check out the cart they already filled
    let checkout = svc.checkout_cart(state.cart).await;
    then!(checkout).rejected_as_invalid_state("customer is suspended");
}
```

The parent (`items_added_to_cart`) established the customer; this node lays the admin's action on top. The admin's own "setup" (merely existing) belongs in the root alongside the other initial data — case 1.

**When you doubt the order is safe — branch it.**
Linearizing silently *assumes* the two actions commute. If you're unsure they do, or want to prove they don't matter, write the reverse order as a **sibling branch**: "admin suspends → customer checks out" beside "customer checks out → admin suspends," two children of one parent. The tree now demonstrates order-independence instead of assuming it — or catches the case where it breaks. Reserve this for orders you actually doubt; don't reverse every independent pair, or the tree explodes.

**If you genuinely need results from two sibling branches** (for example, "two separately checked-out orders are merged into one shipment"), don't fight the tree — restructure so both actions happen in sequence within one lineage: first checkout is one node, second checkout is its child, the merge is the grandchild. The narrative even reads better that way: events in a real system are ordered, too.

---

# It fits — a finite set of test cases

Sometimes there is no state to reuse and no tree to grow — you simply want to verify a known, enumerable set of cases and see each one in the output. A pure function is the classic example: a validation regex, a parser, a price formatter, a serialization round-trip. There are no side effects and no story, so the SHAPE loop has nothing to loop over. That's fine — you're borrowing `testscribe` purely for its readable output, and that is a legitimate, smaller use of the library.

**The testscribe way:**
Write a single standalone node and use a [parameterized check](../tests/basic_checks.rs) to express the input/expectation matrix. Each case becomes one readable row in the output:

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
            NameCase { name: "ALICE-99", accepted: true },
            NameCase { name: "al", accepted: false },          // too short
            NameCase { name: "alice smith", accepted: false }, // no spaces
            NameCase { name: "verylongusernamethatkeepsgoing", accepted: false },
        ])
        .run(|case| validate_user_name(case.name).is_ok() == case.accepted);
}
```

Output:

```text
 | 0.037ms|Given user name validation rules defined
 |       -|  Then user name is accepted or rejected by validation rules
 |       -|  |                           name, accepted |
 |       -|  |                          alice,     true |
 |       -|  |                       ALICE-99,     true |
 |       -|  |                             al,    false |
 |       -|  |                    alice smith,    false |
 |       -|  | verylongusernamethatkeepsgoing,    false |
```

Compare that to a single `test validate_user_name ... ok` line, which hides *which* cases were probed.

**What to keep in mind:**

- The SHAPE loop does **not** apply — there is no state to hook into and nothing to expand. Don't force it.
- Naming still matters: name the node after what it establishes (`user_name_validation_rules_defined`), not after the function (`test_validate_user_name`).
- The statement should describe the rule being verified, so the output reads naturally.

**Don't lose the story it belongs to:**
If the pure function guards a domain action (for example, `validate_user_name` is called by `create_user`), the matrix proves the rules are *correct* — but a *stateful* probe inside the tree proves they are *wired in*. Add one or two representative rejections next to the node that performs the action:

```rust
let bad_name = svc.create_user("bob@example.com", "x").await;
then!(bad_name).rejected_as_invalid_name();
```

A perfect regex that nobody calls passes the matrix test and still ships broken. The matrix verifies the cases; the wiring probe lives in the story.

---

# It doesn't fit

When your test is neither a linear story nor an enumerable matrix, `testscribe` is the wrong tool — and not because of a missing feature, but because of its model. Something falls outside for one of two reasons:

1. **The execution model isn't linear.** `testscribe` describes one ordered sequence of events. The moment outcomes depend on *interleavings* or on *generated* inputs rather than a sequence you wrote down, there's no single story to narrate.
2. **There's no outcome worth describing.** `testscribe`'s whole value is a detailed, readable record of *what happened*. When the interesting result is a single number — or simply "it didn't crash" — that record adds nothing.

When you hit one of these, reach for the dedicated tool. Mixing it into the suite as a degenerate `testscribe` node only obscures both.

## Concurrency

Races, lock ordering, and interleavings have no single linear narrative — the whole point is that many orderings are possible, and you're hunting the bad ones. That's the opposite of a tree, which fixes one order. Use a tool built for it (for example, [`loom`](https://docs.rs/loom) for exhaustive interleaving exploration), or targeted stress tests.

## Property-based testing and fuzzing

These generate their inputs instead of enumerating them, so there is no finite, narratable case list — and a failure is a *generated* counterexample, not a row you wrote. That's neither a story nor a matrix. Use the dedicated runners directly: [`proptest`](https://docs.rs/proptest), [`quickcheck`](https://docs.rs/quickcheck), or `cargo fuzz`.

> A matrix (the section above) and a property test look superficially similar — both probe many inputs. The dividing line is who chooses the inputs: if *you* enumerate a finite set, it's a `testscribe` matrix; if the *tool* generates them, it isn't.

## Benchmarks

The interesting result of a benchmark is the execution time — a number. There's no side effect to assert and no narrative to read, so `testscribe`'s detailed output buys you nothing. Use a benchmarking harness ([`criterion`](https://docs.rs/criterion), `cargo bench`) that measures, compares, and tracks regressions in timing.

---

## Summary

| Your test is… | Fits? | Pattern |
|---|---|---|
| A stateful sequence of events | ✅ a linear story | The test tree + SHAPE loop (the rest of the docs) |
| Time-dependent behavior | ✅ a linear story | Clock in the environment; time passing is the event |
| Independent actions / multiple actors / a "diamond" | ✅ a linear story | One lineage; fold away, merge, or order the actions in sequence |
| A pure function / enumerable cases | ✅ a finite matrix | Standalone node + `params` check (readability only) |
| Concurrency / races | ❌ non-linear | `loom`, stress tests |
| Property-based / fuzz testing | ❌ generated inputs | `proptest`, `quickcheck`, `cargo fuzz` |
| Benchmarks | ❌ no outcome to describe | `criterion`, `cargo bench` |

If a test still feels like it has no home after this page, that's useful signal — [open an issue](https://github.com/fraillt/testscribe/issues) and describe the scenario.
</content>
</invoke>

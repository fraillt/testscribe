## Advanced techniques

Once you're familiar with the general flow of writing test trees, you can start exploring more advanced techniques to improve readability and performance.
The techniques are ordered by relevance. Environments and custom checks typically provide immediate value and are good starting points. State cloning is an optimization whose benefits depend on your specific test structure, so it's best evaluated after gaining experience with the earlier techniques.

> Important: the linked files in this section are primarily **syntax-oriented examples** (how a feature looks and how to call it), not canonical end-to-end modeling examples.
>
> A truly canonical example would require much more domain context and more sophisticated test trees. Here, the goal is to help you quickly understand feature syntax and available patterns.

### 1) Use environments to model state ownership explicitly

**Problem it solves:**
Without a clear boundary, tests mix two very different kinds of state: (1) state produced by business actions, and (2) infrastructure state required to run tests. When those are mixed together, it becomes harder to reason about what the test is actually proving.

**How `testscribe` helps:**
Environments make this distinction explicit:
- **Test state** = business-relevant state produced by test actions and passed to child nodes.
- **Environment state** = infrastructure context that supports execution (database connections, mocked external services, shared clients, etc.).

This is not only a technical extraction of setup code. It is a semantic model of ownership: what behavior produced versus what infrastructure enabled.

**Example:**
- Calling `create_customer(...)` produces `customer_id`; that belongs to **test state** and should be returned from the test node.
- `database_pool`, external service stubs, and shared fixtures belong to **environment state**.
- Child tests should depend on parent test state for behavior continuity, while reading environment as execution context.

Reference: [environment.rs](../tests/environment.rs)

### 2) Add custom checks when assertions are technically correct but hard to read

**Problem it solves:**
Generic assertions can become noisy and low-signal in both code and output, especially for domain-specific validation or nested error variants.

**How `testscribe` helps:**
You can add project-local checks on `VerifyValue` so assertions read like domain language and output becomes clearer for humans.

Equality (`eq`/`ne`) and containment (`contains`, for `Vec<T>` membership and `String`/`&str`
substrings) are built in; reach for a custom check when those don't express the domain concept.

**Example:**
- Instead of long pattern matches on nested errors, add a targeted check like `.has_valid_email(...)` or `.rejected_as_err_kind("...")`.
- Instead of asserting an opaque boolean, add an ordering check like `.sorted_ascending()` or a domain check like `.within_credit_limit()`.

> Naming tip: avoid `is_*`/`to_*` prefixes for custom checks — they consume `self` by value, which triggers clippy's `wrong_self_convention` lint. Outcome wording (`rejected_as_...`, `has_...`) avoids the lint and reads better in test output.

**Writing the check — the `VerifyValueExposed` step:**
A custom check is a trait implemented for `VerifyValue<'_, T>`. Inside the impl you immediately convert it with `VerifyValueExposed::new(self)` to reach the data you need:

- `this.actual_value` — the value under check, as `&T` (a reference, not an owned value).
- `this.var_name` — the variable name, or the alias from `then!(... => alias)`.
- `this.reporter` — call `set_outcome(description, VerifyOutcome::Success | VerifyOutcome::Failure { details })`.

Why two types? `VerifyValue` deliberately exposes **no** public fields or methods, so the only things IDE autocomplete offers on a `then!(...)` result are the checks themselves (`.eq`, `.contains`, your custom ones) — never internal plumbing. `VerifyValueExposed` is the escape hatch, used *only inside* a custom check's implementation, to reach those internals.

Reference: [custom_checks.rs](../tests/custom_checks.rs)

### 3) Clone state and environment to avoid redundant re-execution

**Problem it solves:**
By default, every leaf test starts from a clean, independently-built state: to run a leaf, `testscribe` re-executes its entire ancestor chain from the root. That keeps branches isolated and correct, but it means a node's body runs **once per leaf in its subtree** — in a tree with 20 leaves the root runs **20 times**, a node with 5 leaves beneath it runs 5 times. When the repeated work is slow, this re-execution dominates the suite's runtime. It bites at either end of the chain: a costly **initial setup** near the root (booting Postgres via testcontainers, seeding fixtures) that every leaf pays for, or a heavy **node deeper in the tree** (a bulk import, an expensive computation, a slow external call).

**How `testscribe` helps:**
Mark a node `cloneable` (or `cloneable_async`): its body runs **once**, and each downstream branch continues from a snapshot of its result instead of rebuilding it. A snapshot copies *both* the node's returned test state and its environment — you clone both or neither, so both must be cloneable: the state via `Clone` / `CloneAsync`, the environment via its own `Clone` / `CloneAsync` impl. Use the `_async` variant when either snapshot needs `.await` (for example, cloning a database).

The node's result is treated as a read-only **template**: it runs once, and then *every* branch that continues from it — including the first — runs on its own independent snapshot. A node with a single continuation is snapshotted too, even though it could in principle reuse the original directly; cloning unconditionally keeps behavior consistent and the model simple — a branch never has to wonder whether it received the original or a copy. The practical payoff is the next section: because the template is never handed to a test, **you never have to keep it usable after snapshotting** — you only have to be able to snapshot it.

**Examples:**
Cloning in-memory values is trivial — derive both, `#[derive(Clone, CloneAsync)]`, and you're done. The cases worth walking through are external state and the edges, below.

#### Cloning with external state

Real-world tests often involve **external state**: databases, files, TCP connections, global Rust objects, etc. There the `Clone` (or `CloneAsync`) implementation must ensure siblings receive **independent copies** of that external state, not shared references to the same mutable resource.

The general pattern is:
1. Create a new instance of the external resource from the existing one.
2. Return a new handle pointing to the new instance.

**Database example (PostgreSQL):**
Implement `CloneAsync` for your environment to create a new database from a template:
```sql
CREATE DATABASE new_random_name WITH TEMPLATE current_db_name;
```
Then return a new connection pool connected to the freshly cloned database. Each sibling gets its own independent database.

**File example:**
Create a new file with a random name, copy contents from the original, and return a handle to the new file. Each sibling operates on its own copy.

#### Cloning resources that are in use

`clone_async` runs between test executions. Because the node's result is a read-only template that no test runs on again — every branch gets its own snapshot — there is exactly **one** obligation that in-memory clones never face:

**Quiesce the source before snapshotting.** Most snapshot mechanisms require exclusive access to the source. PostgreSQL refuses `CREATE DATABASE ... WITH TEMPLATE` while *any* session is connected to the template database; copying a file is only safe with no open writers. Tests never run concurrently with a clone, so nothing is actively *using* the resource at that moment — but long-lived handles (connection pools, file descriptors) keep sessions open *between* tests, and they must be released before snapshotting. For example the PostgreSQL recipe is simple — terminate/close sessions, then snapshot using `CREATE DATABASE new_random_name WITH TEMPLATE template_db`.

The template's own pool can stay severed — nothing reconnects to it. Run the snapshot commands over a separate **admin handle** attached to a neutral resource (for PostgreSQL: the `postgres` maintenance database), so the clone operation does not itself hold the source open.

A complete working PostgreSQL environment (testcontainers + one database per branch) is in [checkout_flow.rs](https://github.com/fraillt/testscribe/blob/main/examples/checkout-domain/tests/checkout_flow.rs).

#### Pushing an un-cloneable environment below the branch point

Sometimes one environment is hard or impossible to clone, but a simpler base still is. Because environments can *transform* as they flow down the tree — each declares a `type Base`, and a child can request a different environment built from its parent's via [`Environment::create`](../tests/environment.rs) — you can keep the **cloneable** base as the environment at the branch point (so only it is snapshotted) and introduce the un-cloneable environment **below** the branch, in the children. Each child builds it fresh from the cloned base, so it is created per-branch and never has to be cloned itself.

#### When cloning is not possible

Sometimes external state cannot be cloned at all. For example, if test state directly exposes a file path or a fixed resource name that other systems depend on, creating a copy under a different name would break the contract.

In these cases, do not use cloning. Instead, rely on the default behavior: parents re-execute for each branch, producing fresh state each time. Make sure the environment's `create` function cleans up any leftover resources from previous runs (for example, deleting a well-known test file if it already exists), so that each execution starts clean.

Reference: [clone_state_and_environment.rs](../tests/clone_state_and_environment.rs)

### 4) Use parameterized checks for behavior matrices

**Problem it solves:**
When one action is repeated across many state variants, writing separate tests can create duplication and make coverage harder to scan.

**How `testscribe` helps:**
Parameterized checks let you define a compact matrix and verify each row with clear output.

**Example:**
Imagine actions that always happen (close payment, notify status change, display in reporting), but expected outcomes depend on payment result:

|                          | accepted             | failed                   |
|--------------------------|----------------------|--------------------------|
| close payment            | set state - accepted | set state - failed       |
| report status change     | notify - client      | notify - operations team |
| show in reporting system | show as transferred  | not shown in the system  |

Reference: [parameterized_tests.rs](../tests/parameterized_tests.rs)

### 5) Build a custom runner when default execution/reporting is not enough

**Problem it solves:**
Sometimes teams need custom execution control, custom reporting, or integration with external orchestration systems.

**How `testscribe` helps:**
You can use internals to build your own runner and define how tests are selected, repeated, and reported.

**Example:**
When you write `#[testscribe(standalone)]` you're already using `testscribe-standalone` crate, but this crate exposes more functions so you could write your own test harness.
Lastly there's experimental support for `detached` test runner (`testscribe-detached` crate), meaning that test execution is controlled by external tools.

References:
- [custom_test_runner.rs](../tests/custom_test_runner.rs)

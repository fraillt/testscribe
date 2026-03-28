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

Reference: [environment.rs](../examples/features-showcase/tests/environment.rs)

### 2) Add custom checks when assertions are technically correct but hard to read

**Problem it solves:**
Generic assertions can become noisy and low-signal in both code and output, especially for domain-specific validation or nested error variants.

**How `testscribe` helps:**
You can add project-local checks on `VerifyValue` so assertions read like domain language and output becomes clearer for humans.

**Example:**
- Instead of searching through a collection with repetitive logic each time, add a custom `.contains(...)` check.
- Instead of long pattern matches on nested errors, add a targeted check like `.is_email_valid(...)` or `.is_err_kind("...")`.

Reference: [custom_checks.rs](../examples/features-showcase/tests/custom_checks.rs)

### 3) Use state cloning when integration setup dominates execution time

State cloning is an **optimization technique** to reduce test execution time. Without cloning, `testscribe` re-executes all parent nodes for each child branch, which ensures every branch starts from clean state. This is safe and correct by default. Cloning avoids that re-execution cost by snapshotting state at a branching point and reusing it for each sibling.

**When cloning is not worth the effort:**
If your test tree is small or parent execution is fast, cloning adds complexity without meaningful benefit. Start without it and introduce cloning only when re-execution time becomes a bottleneck.

**Problem it solves:**
Expensive setup (for example booting Postgres via testcontainers) can make large test trees slow, even when each node itself is fast.

**How `testscribe` helps:**
Clone test/environment state at useful points and branch from those snapshots instead of rebuilding everything from scratch.

#### Cloning with external state

Cloning in-memory values is straightforward, but real-world tests often involve **external state**: databases, files, TCP connections, global Rust objects, etc. The `Clone` (or `CloneAsync`) implementation must ensure siblings receive **independent copies** of that external state, not shared references to the same mutable resource.

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

#### When cloning is not possible

Sometimes external state cannot be cloned. For example, if test state directly exposes a file path or a fixed resource name that other systems depend on, creating a copy under a different name would break the contract.

In these cases, do not use cloning. Instead, rely on the default behavior: parents re-execute for each branch, producing fresh state each time. Make sure the environment's `create` function cleans up any leftover resources from previous runs (for example, deleting a well-known test file if it already exists), so that each execution starts clean.

Reference: [clone_state_and_environment.rs](../examples/features-showcase/tests/clone_state_and_environment.rs)

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

Reference: [parameterized_tests.rs](../examples/features-showcase/tests/parameterized_tests.rs)

### 5) Build a custom runner when default execution/reporting is not enough

**Problem it solves:**
Sometimes teams need custom execution control, custom reporting, or integration with external orchestration systems.

**How `testscribe` helps:**
You can use internals to build your own runner and define how tests are selected, repeated, and reported.

**Example:**
The detached runner direction explores remote-controlled execution where a server can control run strategy and presentation, potentially including test history and richer comparison workflows.

References:
- [custom_test_runner.rs](../examples/features-showcase/tests/custom_test_runner.rs)
- [crates/detached](../crates/detached/)

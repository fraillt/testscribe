# testscribe 0.2.0 - feedback from first use

Written after using testscribe for about 35 test functions across four crates of another project
(a blockchain scanner on Postgres), by an AI agent that had not used the library before. Each
problem carries what was done about it since.

| # | problem | status |
| --- | --- | --- |
| 1 | a run reported `ok` although nothing could run | fixed |
| 2 | output of parallel roots interleaves | fixed |
| 3 | no check for "is empty" | added |
| 4 | readable output needs a helper per file | documented as an idiom |
| 5 | enum variants with payloads | added |
| 6 | pure functions have no action | was documented already |
| 7 | small ergonomics | open |

## What worked well

- **The agent guide shipped inside the crate.** `docs/llms.md`, `guidelines.md` and
  `testing_checklist.md` were readable offline from the cargo registry. With only those and the
  examples in `tests/`, the first tests compiled and passed on the first run.
- **The tree fits stateful domains.** A deposit's life is a chain of states - detected, confirmed,
  reverted, mined again, confirmed again - with branches for orphaned, restart and source offline.
  Each test is 5-10 lines because the parent built the state. One scenario is five levels deep
  with no setup duplication.
- **The narrative output is a review tool.** Reading it line by line after the first green run
  showed that every child ran and whether each line said what was meant. It caught two assertions
  that dumped a whole struct.
- **Event-style names enforce one action per test.** When a name would not read as an event, the
  test was usually doing two things.

## Problems

### 1. A run reported `ok` although nothing could run (serious)

Setup: a root test `#[testscribe(standalone(tokio::test))]` with four leaf paths below it, each
taking `Env<Db>`. `Db::create` connects to Postgres and unwraps. Postgres was down.

Observed, twice: the run took 120 s (four paths x the 30 s connection timeout), printed no
Given/When/Then lines, and ended with `test result: ok. 1 passed; 0 failed`.

A third run, cut off after 100 s, showed panic backtraces pointing into the root test and a line
prefixed with `?`: `?| 0.000ns|  When scanner restarted and two more blocks scanned`.

Not investigated further at the time. The guess was that a panic inside `Environment::create`
is not counted as a failure of the tree.

**Fixed.** The guess was right. A panic in `Environment::create` made `execute_test` return a
skip without telling the logger anything: the test that needed the environment was neither
started, failed nor skipped, the tests below it were skipped, and a tree in which nothing failed
counts as passed. Two more ways into the same hole were found and closed with it: a panic while
state or environment is cloned for the next test, and a test that passes on its first run and
panics when executed again for a later branch. The test that could not run is now reported as
failed, with the reason:

```text
!| 8.018μs|Given deposit of 50 usdt scanned in block 101
?| 0.000ns|  When two more blocks scanned and deposit confirmed
...
failures:
  tests/stored_scanner.rs:17:0  stored_scanner::DepositOf50UsdtScannedInBlock101  test should not panic
      environment `stored_scanner::support::Db` could not be created: called `Result::unwrap()` on an `Err` value: PoolTimedOut
```

The 120 seconds had their own cause: a test that failed was executed again for every leaf below
it, so the 30 second timeout was waited for four times. A failed or skipped test is now executed
once.

Found on the way: panic messages never reached the report. A panicking test was reported with
`<unknown msg>` and a panicking check with `panic: no info`, because the boxed panic payload
itself was inspected instead of what it holds. Fixed too.

Covered by `crates/self-tests/tests/panics.rs`.

### 2. Output of parallel roots interleaves

With several standalone roots in one file, the lines of different trees mix, e.g. a `Then` of one
tree lands between a `When` and its `Then` of another. The narrative is readable only with
`--test-threads=1`. Buffering the output per root and printing it when the root finishes would
fix it.

**Fixed.** Without `--nocapture` a tree was already printed in one piece; with it, every tree
wrote straight to stdout. A standalone tree is now printed when it has finished, unless it is
certain to run alone (`--test-threads=1`), where it still prints test by test.

### 3. No check for "is empty"

`then!(events).eq(Vec::<String>::new())` was written about ten times. An `.is_empty()` check
would read better in code and in output ("Then events is empty").

**Added:** `then!(events).is_empty()` and `.is_not_empty()`, for collections and texts
(`CheckEmpty`, in the prelude). A collection of your own takes part by implementing `MaybeEmpty`.

### 4. Readable output needs a helper per file

Comparing structs prints their whole `Debug`, which is unreadable for anything nested. Every test
file grew a `told()` function that turns domain values into short sentences
(`"Confirmed Incoming 50.000000 in block 101"`), and assertions compare `Vec<String>`. The result
reads well, but it is boilerplate. Possible help: a way to give `then!` a formatter, or to assert
through `Display`.

**Documented as an idiom**, with nothing added to the library. The sentence a domain value is
told as is the project's choice, and comparing through `Display` would not have helped here:
integration tests are a crate of their own and cannot implement `Display` for the types of the
crate under test. What the project needs is one `Told` trait (`fn told(&self) -> String`) and one
`reads` check for every `T: Told`, written once in the module its test files share, instead of a
helper per file:

```text
then!(transfers[0] => deposit).reads("Confirmed Incoming 50.000000 in block 101");

 |       -|  Then deposit reads Confirmed Incoming 50.000000 in block 101
```

A list is checked item by item, so that a check stays one line of output. See "Telling domain
values" in `docs/advanced_techniques.md` and `tests/custom_checks.rs`.

### 5. Enum variants with payloads

For "this step was a rollback of block 102" the shortest form was
`then!("blocks 102 and 101 were dropped").run(|| matches!(step, Step::RolledBack { .. }))`,
which the guidelines discourage. A custom check is the idiomatic answer but is heavy for one or
two uses. A built-in `.matches(pattern)` style check would cover it.

**Added:** `then!(step).is("a rollback", |step| matches!(step, Step::RolledBack { .. }))` and
`.is_not(..)` (`CheckIs`, in the prelude). A pattern cannot be passed to a method, so the check
takes a description and a predicate: it is reported as `Then step is a rollback`, and shows the
actual value when it fails. A predicate written in many tests is still better as a custom check.

### 6. Pure functions have no action

A conversion function has inputs and outputs, no side effects. It became one standalone test
holding a table of `then!` lines, which works, but sits outside the "action with side effects"
model the guides teach. A short example of this case in `llms.md` would remove the doubt.

**Was documented already.** `docs/does_it_fit.md` has this exact case: a standalone node with a
`params` check, used for its readable output only. `llms.md` points there in one line of its
"Core information"; the agent did not follow it. A short example in `llms.md` itself might still
help.

### 7. Small ergonomics

- A child that only reads its parent's state still has to choose between `s: Given<..>` and
  `mut s: Given<..>`; getting it wrong is an `unused_mut` warning, which fails `-D warnings`.
- Changing the state type between parent and child needs `into_inner()` first; it took a moment
  to find.

**Open.** Not looked at.

## Not measured

Each leaf path re-runs its parents with a fresh environment. With a local Postgres the tree of
eight tests takes 0.5 s in total. With one container per environment it is probably much slower
unless state and environment cloning is used, which was not tried.

# Design notes

Why the frontend is the way it is: the decisions that are not obvious from the code, what was
left out on purpose, and what is known not to work. When something behaves in a way you did not
expect, look here first: it is either a decision (and then it says why), a known gap, or a bug.

[README.md](README.md) says how to use it. This file says why.

## The shape of it

```text
 test binaries ──ws /tests──▶  server  ◀──ws /ui── UI
                                 ├── sessions: every message of every test binary, in memory
                                 ├── batches: what every test said the last time it ran (SQLite)
                                 └── commands: `cargo test`, when the UI asks for a run
```

- **The UI talks to the server over one WebSocket and nothing else.** No HTTP API. The desktop
  window loads the UI from its own origin, where a `fetch` to the server would need CORS, and a
  WebSocket does not. So requests that have an answer (`get_batch`, `diff`) are messages too, and
  the answer goes to the socket that asked.
- **Two views of the same tests, built twice.** A *session* (a "run" in the sidebar) is the raw
  messages of one test binary; the UI builds the narrative from them in `ui/src/lib/narrative.ts`.
  A *batch* is built by the server from the same messages in `server/src/outcome.rs`. Both follow
  the rules of the terminal printer (`crates/standalone/src/logger/printer.rs`). **These are three
  implementations of the same rules**, and nothing but care keeps them the same. If the live view
  and the page of a batch disagree about a test, that is where to look.
- **The server reads messages as JSON values, not as the types of the protocol.** It depends on no
  testscribe crate. A field it does not know is passed on to the UI untouched; a field that is
  missing reads as empty. So an older test binary works with a newer frontend, and a typo in a
  field name fails silently.

## What a batch is

- **A test is known by its test binary and its path**: the names from the root of its tree down
  to it, each as `module::path::TestName`. The binary is the file name without the hash cargo
  adds (16 hex digits after the last `-`).
  - Renaming a test, or moving it to another module or under another parent, is a deleted test
    and an added one.
  - Two packages with a test binary of the same name (`tests/basic.rs` in both) and the same root
    name would share entries. Not handled. The package is not part of the key, because its path
    differs between machines and checkouts, and batches are meant to be compared across them.
- **The outcome is text**: what the terminal prints for the test, without times, without file
  names and line numbers. That is what makes "moving code around changes nothing" true. A failed
  line starts with `✗`, so a test that starts failing reads differently even if its messages do
  not change. Panic messages are part of the text; they are the message only, without a location.
- **A test with parameters, or below one, is one entry.** Its text is all its runs, one after the
  other, separated by an empty line. A run does not say which parameter of a test *above* it it
  belongs to, only its own; the order is the order of execution, which is stable.
- **A test executed again** (for a branch below it) is told only for its first execution, and
  again the first time a later execution fails, marked `(executed again)`. This is the rule of the
  terminal printer. Such a test is remembered as "failed again", see the run modes below.
- **Statuses**: `passed`, `failed`, `skipped` (a test above it panicked, so it could not run) and
  `not_run`. A test with several runs is `failed` if any failed, else `skipped` if any was.
- **A batch knows every test its binaries announce**, not only the ones that ran. A test binary
  sends all its trees when it connects; each test gets a row, `not_run` until it runs. Without
  this, "did not run" and "is not in the code" look the same, and a comparison calls every test
  that did not run deleted. (That was the first version, and `cargo test` stopping at the first
  failing binary made half a batch "deleted".)
- **Tests that are no longer announced are forgotten** when their binary connects, in every run
  mode. This is the one place where a run changes tests it did not run. The reasoning: the batch
  describes the code, and the test is not in the code.
- **A test binary that is gone is never forgotten by itself**, because it never connects again.
  "forget" on the page of the batch is the way. See "what one `cargo test` is" below for why
  this is not automatic.
- **"Everything" starts a tree over when the tree starts**, not when it ends: all its tests go
  back to `not_run`, then each is written as it finishes. A run that is killed halfway leaves the
  rest `not_run`. The alternative, replacing at the end, would keep old outcomes next to new ones
  for the length of the run.
- **Every test is written when it finishes**, so the page of a batch follows a run. The server
  tells the UIs that batches changed at most every 300 ms while tests report, and always when a
  tree finishes. On each such message the UI **asks for the whole batch again**. Simple, and
  fine for hundreds of tests; with many thousands it will be slow, and the fix is to send what
  changed.

### Where it is stored

- **Named batches are in a SQLite file; `default` is in a SQLite database in memory** with the
  same schema. A batch name picks the connection, so `default` cannot be stored by accident and
  one code path serves both. Comparing works across the two because the comparison is done in
  Rust, on the tests of both sides, not in SQL.
- `Frontend::new()` has no file: there, named batches are in memory too. The tests use this.
- **The database is changed by adding columns** (`ADDED_COLUMNS` in `store.rs`), checked on
  every start. There is no schema version. Removing or changing a column needs more than this.
- WAL and `synchronous = NORMAL`, because every test is its own write. A crash of the machine
  can lose the last writes; a crash of the server cannot.
- Times are stored as microseconds in three columns. They are not part of what is compared.

## What runs: the three modes

- **The mode belongs to the batch and stays** until it is changed. It is not "the next run only".
- **Precedence**: `--exact` or `TESTSCRIBE_ONLY` on the command line (the frontend is not even
  asked), then `TESTSCRIBE_RUN` of the run, then the mode of the batch.
- **A selection is a list of exact paths.** Ticking a test in the UI ticks everything below it,
  as a convenience; the tests *above* a selected test run because they have to, and are written
  over, because they did run. Tests that are selected and no longer exist are ignored, and stay
  in the selection.
- **New and failed** runs every test that is not `passed`: `failed`, `skipped`, `not_run`.
  - A tree of which nothing has run yet runs as "everything".
  - **A test that failed only when executed again** would pass if it ran alone, since alone it is
    executed once. So everything below it runs too. This runs more than the minimum: the server
    does not know which branch the failing execution was for.
- **When nothing of a tree is to run**, the test binary is told so, runs nothing, and reports the
  tree as finished successfully. `cargo test` prints `ok` for it. In the live view of that run
  the tree shows as passed with no tests. Cosmetic, and misleading.
- **A test binary waits 5 seconds for the answer** and then runs everything
  (`crates/detached/src/connection.rs`). The server does not know that happened, so the tree is
  not started over; the tests that ran are written over one by one, which comes to the same.

### What one `cargo test` is

A test binary sends the id of the process that started it (`invocation`, Unix only). The
binaries of one `cargo test` have it in common. **It is only used to draw a line between runs in
the sidebar.**

It was meant for more: to reset a whole batch when "everything" runs, and to forget binaries that
did not report. It cannot do that safely. A run of `cargo test -p one-package`, or with a filter,
or one that cargo cut short at the first failure, looks exactly like a run of everything in
which the other binaries are gone. So a batch is only ever changed for binaries that did report.

## Comparing

- A test with an outcome only in the newer batch is **added**; with outcomes that read
  differently, **modified**; with an outcome only in the older one, **deleted** if the newer batch
  knows its binary and not the test.
- **Not run**: the older batch has an outcome, and the newer batch either knows the test as
  `not_run`, or does not know its binary at all. The second case is a guess: the binary may have
  been deleted. It is the safer guess, since the usual reason is a run that did not get that far.
- A test that is `not_run` in the older batch and has an outcome in the newer one is "added",
  though it was in the code before. Accurate about outcomes, slightly off about tests.
- The changes are ordered by binary and by path as text, which puts a test after the test above
  it, but is not the order the tests are written in.
- Lines are compared with a plain longest-common-subsequence table: quadratic in the number of
  lines of one test. Fine for tests; not for a test that prints tens of thousands of lines.
- **`diff` on the command line fails when anything differs, "not run" included.** As a gate in CI,
  a test that silently did not run must not pass.
- **Times** are compared only for tests that say the same on both sides. Differences of
  microseconds are noise, and nothing filters them: the list is sorted by the size of the
  difference, and the reader decides.

### A batch as a file

- The file has binary, path, status and text of the tests that have an outcome. No times, no
  source positions, no `not_run` rows, no git state: **nothing that differs between two runs of
  the same code**, so the file only changes when behaviour does.
- `import` replaces the whole batch. An imported batch has no announced tests, so compared
  *against* (as the newer side) it calls missing tests deleted, never "not run".

## Executions and times

- **Test time** is from a test's `Started` to its `Finished`. **Setup time** is from the end of
  the test before it to its `Started`. Nothing in the protocol says "setup"; it is derived, as
  the gap. In that gap the runner creates the environment of the test and copies the state and
  environment it starts from. It also contains whatever else the runner does between two tests.
- So **test and setup time of all tests of a tree add up to the time of the last `Finished`**.
  Not counted: what happens after the last test (dropping state, reporting), which the protocol
  does not time.
- The setup of the **first test of a tree** starts at zero, so it includes the start of the
  runner.
- **Executions** count every execution, also the silent ones. A test without parameters that is
  not cloneable runs once per leaf below it.
- **After a partial run, times are mixed**: the tests that ran have the counts and times of that
  run, the others of an earlier one. "With its branches" then adds up numbers of different runs.
  This is visible in a comparison as `3× → 1×`.
- A row shows test + setup time over all executions, in both views. In the live view a test that
  appears on several rows (once per parameter) shows the same total on each.
- The hint "would save about …" assumes that a test which could be copied runs once, and that
  copying is free. It is an estimate of the upper bound.
- Hidden by default; "show times" is remembered by the browser, not by the batch.

## Running the tests from the UI

- **This is remote code execution by design.** The command of a batch is any shell command, and
  the UI sets it. Three things stand in front of it:
  1. It is off unless the server was started with it allowed (`Frontend::allow_commands`). The
     binary allows it when it listens on `127.0.0.1`, `localhost` or `[::1]`, and not otherwise;
     `--allow-run` and `--no-run` override that.
  2. **The `/ui` socket checks the `Origin` header.** A browser lets any web page open a WebSocket
     to `127.0.0.1`; without this check, any page you visit could run commands here. Let in: no
     origin (not a browser), the address the request came to, anything on `localhost`,
     `127.0.0.1` or `[::1]` on any port (the dev server of the UI; also any other local web
     server), and the desktop window.
  3. Nothing else. **There is no authentication anywhere.** With `--listen 0.0.0.0`, anyone who
     reaches the port can read and delete batches, and feed it fake test results on `/tests`,
     which has no origin check since test binaries are not browsers.
- The command runs through `sh -c` in a process group of its own, so that "stop" ends `cargo`
  and the test binaries it started (`kill -TERM` to the group). On Windows it is `cmd /C`, and
  stop kills only the shell. **Windows is not tested at all.**
- It gets `TESTSCRIBE_FRONTEND` (the address the server was told it listens on) and
  `TESTSCRIBE_BATCH_ID` (not for `default`); a `TESTSCRIBE_RUN` of the server's own environment is
  removed, so that the mode set in the UI holds.
- One command per batch at a time. Its last 2000 lines are kept in memory and are gone with the
  server. Lines are passed to the UIs one message each; a UI that falls 4096 messages behind is
  disconnected, reconnects, and gets everything again.
- **The directory** is the nearest directory with a `Cargo.lock` above the package of the first
  test binary that reported to the batch. It is set once and then only changed by hand. A
  workspace without a `Cargo.lock` falls back to the package directory.
- The default command is `cargo test --no-fail-fast`, without `--features detached`. A project
  that has the feature behind a flag must put it into the command of the batch.

## What is known about the sources

- **The server asks git, not the test binary**: it runs `git` in the workspace directory when a
  test binary connects (once per 5 seconds per directory). That keeps git out of the published
  crates. It is **wrong when the tests run on another machine** than the server: then the
  directory does not exist there, and commit, branch, links to sources and "run" have nothing.
- The batch keeps the state of the sources of the *last* binary that reported. A batch run in
  parts, on different commits, shows the last one.
- **Links to sources** are built in the browser from a template (`vscode://file/{path}:{line}`,
  kept by the browser) and the workspace directory plus the file name the test announced. They
  work when the browser is on the machine that has the sources.

## The UI

- State is in memory. **What is shown is not in the URL**: a reload goes back to the newest run.
- "show times" and the editor link are in the browser's local storage, per browser.
- **Sessions are never dropped by themselves.** Every message of every test binary stays in the
  memory of the server, and is sent to every UI that connects, until "clear finished". A long
  day of runs makes the first load slow.
- The filters on the page of a batch show the matching tests and the tests above them. Search
  looks in the name and the text of a test, every word on its own.
- "slowest first" is a flat list; the tree is gone there on purpose.

## What is tested, and what was only tried

- **Tested** (`cargo test` in `frontend/`, 25 tests): the text of an outcome and its timing; the
  store (announcing, partial and whole runs, forgetting, storing, the comparison rules, files);
  and through real WebSockets: relaying, the three modes, `TESTSCRIBE_RUN`, writing while a tree
  runs, the "failed again" rule, running and stopping a command, the origin check.
- **Tried by hand** against the tests of the `testscribe` crate, in a browser: everything on the
  page of a batch, the comparison with its times, the live view.
- **Compiled only**: the desktop shell. It was never started.
- **Never run**: Windows; a server on another machine than the tests; batches with thousands of
  tests; two UIs changing the same batch at once (the last one wins, without telling the other).
- The UI has no tests of its own. It is type-checked.

## Known gaps, in one place

1. Three implementations of how a test reads (terminal, live view, batch).
2. Two packages with equally named test binaries and roots share entries.
3. A deleted test binary stays in a batch until "forget".
4. A tree told to run nothing shows as passed in the live view.
5. Times mix runs after a partial run.
6. No authentication; `--listen` on anything but this machine exposes everything.
7. Stop on Windows kills only the shell.
8. Git state, source links and "run" assume server and sources on one machine.
9. The whole batch is sent again on every change.
10. Sessions grow without bound until cleared.
11. The view is not in the URL.
12. A test that moves or is renamed is a deleted and an added test; nothing follows it.

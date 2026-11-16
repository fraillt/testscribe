# testscribe frontend

**Experimental.** A place to look at test runs: test binaries report to it while `cargo test`
runs them, and it shows every tree as the narrative the terminal prints, live.

```text
 test binaries ──ws /tests──▶  server  ◀──ws /ui── UI (browser)
                                 └── serves the built UI on the same port
```

- `server/` — the server, a Rust crate. It keeps a session per test binary with its messages in
  order, answers a tree that asks what to run, and passes everything on to the UIs connected to
  it. It keeps the outcome of every test in batches, stored in a SQLite database. It can be used
  as a library, and comes with a binary, `testscribe-frontend`.
- `ui/` — the UI, Svelte and TypeScript. It talks to the server over a WebSocket and nothing
  else, so it is the same UI in a browser and, later, in a desktop window.

`ui/src-tauri` is a desktop shell (Tauri) around the two: `npm run tauri dev` in `ui/`. It needs
the libraries of a webview (`libwebkit2gtk-4.1-dev` on Debian and Ubuntu).

[design.md](design.md) says why things are the way they are, and what is known not to work.

## Running it

```text
cd frontend/ui && npm install && npm run build
cd frontend && cargo run            # listens on 127.0.0.1:7357 and serves the UI there
```

Then, in a project whose `testscribe` dependency has the `detached` feature:

```text
TESTSCRIBE_FRONTEND=127.0.0.1:7357 cargo test
```

and open <http://127.0.0.1:7357>.

`cargo run -- --listen 0.0.0.0:7357` listens for other machines too, `--ui <dir>` serves a
UI built elsewhere, and `--db <file>` says where the batches are stored (`testscribe-batches.sqlite`
in the current directory, unless told otherwise).

## Batches

A batch is what every test in it said the last time it ran: its outcome as the terminal prints
it, without the times and without where the test and its checks are in the sources. A run names
its batch with `TESTSCRIBE_BATCH_ID`, which can be any text:

```text
TESTSCRIBE_FRONTEND=127.0.0.1:7357 TESTSCRIBE_BATCH_ID=before-refactoring cargo test --no-fail-fast
```

A run that names none belongs to the batch `default`. That one is kept in memory only: it is
never stored, and gone when the server stops. Everything else works the same for it.

A batch knows every test its test binaries announce, also the ones that have not run: those are
"not run". The page of a batch shows what its tests say while they run, which sources they last
ran on (the git branch and commit, and whether there were changes that are not committed), and
it can be narrowed down to the tests that did not pass, the ones that have not run, or the ones
a search finds. A test links to its place in the sources; "open sources with" in the sidebar
says which editor that opens (VS Code, unless told otherwise).

Without `--no-fail-fast`, cargo stops at the first test binary that fails. The binaries after it
do not report, and their tests stay as they were, or "not run" in a new batch.

### Running a batch again

A batch remembers what to run the next time its tests ask. It is set on the page of the batch,
and it holds until it is set to something else:

- **everything** — the default. Every tree that runs starts over: what the batch knew of its
  tests is forgotten.
- **selected tests** — only the tests picked in the UI run, and the tests above them, which
  they start from.
- **new and failed** — only the tests that have not run yet, and the ones that did not pass.
  Made for development: run, fix, run again, until nothing is left to run.

Whatever ran, the batch only writes over the tests that actually ran. The other tests stay as
they were. A test that is no longer in the code is forgotten when its test binary reports again;
a test binary that is no longer there never does, and is forgotten with "forget" on the page.

`TESTSCRIBE_RUN=all` or `TESTSCRIBE_RUN=new-and-failed` on a run comes before what the batch is
set to, for that run. A selection made on the command line, with `--exact` or `TESTSCRIBE_ONLY`,
comes before both: the frontend is then not asked, and the batch keeps what ran.

### Running the tests from the UI

**run** on the page of a batch has the server run the tests: it executes the command of the
batch, `cargo test --no-fail-fast` unless "command…" says otherwise, in the directory of the
batch, with `TESTSCRIBE_FRONTEND` and `TESTSCRIBE_BATCH_ID` set. What the command prints is shown
on the page, and **stop** ends it. The directory is the workspace of the first test binary that
reported; a batch made with "new batch…" has to be told.

Whoever reaches the UI can run any command on the machine of the server this way. So it is
only there when the server listens on this machine alone, as it does unless told otherwise.
`--allow-run` allows it wherever the server listens, and `--no-run` forbids it.

### Executions and times

The times are hidden until "show times" is ticked in the sidebar; the browser remembers that.
Then every test shows how long it took, on the page of a batch and in a run as it happens, and
how often it was executed when that was more than once. Click the time for the test with its
branches, the tests below it: for each of them how often it was executed, the time in the test
itself, and its setup time, and all of that summed up. "slowest first" lists the tests of a
batch by the time they took.

A test is executed again for every branch below it that starts from what it built, unless what
it built can be copied. Setup is the time before a test starts, since the test before it ended:
that is where its environment is created, and where the state and the environment it starts
from are copied. So the test and setup times of a whole tree add up to the time the tree took.
The times are those of the last run of each test, and are not part of what batches are compared
by.

### Comparing batches

In the UI, open a batch and pick the one to compare it with. Or, without the server running:

```text
cargo run -- diff before-refactoring after-refactoring
```

Both show the tests that were added, deleted and modified, the modified ones line by line. A
test is known by its test binary and its place in the tree, and it is modified when its outcome
reads differently. Moving code around changes no outcome.

A test that ran in the older batch and has not run in the newer one is told as "not run", not
as deleted: it may well be there. That is a test the newer batch knows and did not run, and
every test of a test binary that did not report to the newer batch at all.

In the UI, "times" compares how long the tests took that say the same in both batches.

### A batch as a file, and in CI

```text
cargo run -- export before-refactoring behaviour.json
cargo run -- import behaviour.json before-refactoring
```

The file has what the tests said and nothing else: no times, no places in the sources. So it
is the same file as long as the tests say the same, and it can be kept next to the code.
`diff` takes such a file wherever it takes a batch, and exits with a failure when anything is
different, a test that did not run included. In CI, that makes a change in what the tests say
something a pull request has to show:

```text
testscribe-frontend --db ci.sqlite &
TESTSCRIBE_FRONTEND=127.0.0.1:7357 TESTSCRIBE_BATCH_ID=ci cargo test --no-fail-fast
testscribe-frontend --db ci.sqlite diff behaviour.json ci
```

## Working on the UI

`npm run dev` in `ui/` serves the UI with hot reload; it connects to a server on
`127.0.0.1:7357`. `npm run check` type-checks it.

## What the server tells the UI

The UI connects to `/ui`. Only a page of the frontend itself is let in: one served by the
server, one from this machine, or the window of the desktop app.

It is first sent everything known:
`{"kind": "snapshot", "sessions": [...], "batches": [...], "can_run"}`. A session is one test
binary, with its `id`, its `batch`, the `workspace_dir` of its tests if they are on the machine
of the server, whether it is still `open`, and its `hello` and `events`: its messages as it sent
them (see the `protocol` module of the `testscribe-detached` crate). A batch is listed with its
`name`, whether it is `stored`, its `mode` (`all`, `selected` or `new_and_failed`), how many of
its tests `passed`, `failed`, were `skipped` and have `not_run`, the `commit` and `branch` it
last ran on and whether the sources were `dirty`, and whether the server is `running` its
tests. `can_run` says whether the server may run tests at all. After that, as it happens:

- `{"kind": "session_opened", "session": {...}}`
- `{"kind": "event", "session": <id>, "event": {...}}`
- `{"kind": "session_closed", "session": <id>}`, when the test binary has exited
- `{"kind": "batches", "batches": [...]}`, when anything about a batch has changed; while tests
  report, a few times a second at most
- `{"kind": "command_state", "batch", "running", "code"}` and
  `{"kind": "command_output", "batch", "lines"}`, for the tests the server runs itself

The UI may send:

- `{"kind": "clear"}` to forget the sessions that are over
- `{"kind": "get_batch", "name"}`, answered with `{"kind": "batch", "name", "batch"}`: the batch
  with its `mode`, its `selected` tests, its `command` and `dir`, the `run` of that command
  (`running`, `code` and the last lines of its `output`), and all its `tests`, each with
  `binary`, `path` (the names from the root down), `status`, `text`, where it is (`file` and
  `line`), and how it ran: `runs`, `test_us` and `setup_us`; `null` if there is no such batch
- `{"kind": "set_run", "batch", "mode", "selected": [{"binary", "path"}]}` to set what the batch
  runs the next time; without `selected`, the tests picked earlier stay picked
- `{"kind": "set_command", "batch", "command", "dir"}`, which also makes a batch that is not there
- `{"kind": "run_batch", "batch"}` and `{"kind": "stop_batch", "batch"}`
- `{"kind": "diff", "from", "to"}`, answered with `{"kind": "diff", "from", "to", "changes", "times"}`:
  for every test that differs its `binary`, `path`, `kind` (`added`, `deleted`, `modified` or
  `not_run`), its status `from` and `to`, and its `lines`, each with the `side` that has it
  (`+`, `-`, or a space for both); and for every test that says the same on both sides, how it
  ran `from` and `to`
- `{"kind": "delete_batch", "name"}` and `{"kind": "delete_binary", "batch", "binary"}`

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

A desktop shell (Tauri) around the two is planned and not there yet.

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

Without `--no-fail-fast`, cargo stops at the first test binary that fails, and the batch does not
hear of the binaries after it.

### Executions and times

On the page of a batch, and in a run as it happens, every test shows how long it took, and how often it was executed when
that was more than once. Click the time for the test with its branches, the tests below it: for
each of them how often it was executed, the time in the test itself, and its setup time, and
all of that summed up.

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
reads differently. Moving code around changes no outcome. `diff` exits with a failure when
anything is different.

### Running a batch again

A batch remembers what to run the next time its tests ask. It is set in the UI, on the page of
the batch, and it holds until it is set to something else:

- **everything** — the default. Every tree that runs replaces what the batch knew of it.
- **selected tests** — only the tests picked in the UI run, and the tests above them, which
  they start from.
- **new and failed** — only the tests the batch does not know yet, and the ones that did not
  pass. Made for development: run, fix, run again, until nothing is left to run.

Whatever ran, the batch only writes over the tests that actually ran. The other tests stay as
they were. A test that is no longer in the code is forgotten when its test binary reports again.

A selection made on the command line, with `--exact` or `TESTSCRIBE_ONLY`, comes first: the
frontend is then not asked, and the batch keeps what ran.

## Working on the UI

`npm run dev` in `ui/` serves the UI with hot reload; it connects to a server on
`127.0.0.1:7357`. `npm run check` type-checks it.

## What the server tells the UI

The UI connects to `/ui` and is first sent everything known:
`{"kind": "snapshot", "sessions": [{"id", "batch", "hello", "events", "open"}], "batches": [...]}`.
A session is one test binary; `hello` and `events` are its messages as it sent them (see the
`protocol` module of the `testscribe-detached` crate). A batch is listed with its `name`, whether
it is `stored`, its `mode` (`all`, `selected` or `new_and_failed`), and how many of its tests
`passed`, `failed` and were `skipped`. After that, as it happens:

- `{"kind": "session_opened", "session": {...}}`
- `{"kind": "event", "session": <id>, "event": {...}}`
- `{"kind": "session_closed", "session": <id>}`, when the test binary has exited
- `{"kind": "batches", "batches": [...]}`, when anything about a batch has changed

The UI may send:

- `{"kind": "clear"}` to forget the sessions that are over
- `{"kind": "get_batch", "name"}`, answered with `{"kind": "batch", "name", "batch"}`: the batch
  with its `mode`, its `selected` tests and all its `tests`, each with `binary`, `path` (the names
  from the root down), `status`, `text`, and how it ran: `runs`, `test_us` and `setup_us`; `null`
  if there is no such batch
- `{"kind": "set_run", "batch", "mode", "selected": [{"binary", "path"}]}` to set what the batch
  runs the next time; without `selected`, the tests picked earlier stay picked
- `{"kind": "diff", "from", "to"}`, answered with `{"kind": "diff", "from", "to", "changes"}`:
  for every test that differs its `binary`, `path`, `kind` (`added`, `deleted` or `modified`),
  its status `from` and `to`, and its `lines`, each with the `side` that has it (`+`, `-`, or a
  space for both)
- `{"kind": "delete_batch", "name"}`

# testscribe frontend

**Experimental.** A place to look at test runs: test binaries report to it while `cargo test`
runs them, and it shows every tree as the narrative the terminal prints, live.

```text
 test binaries ──ws /tests──▶  server  ◀──ws /ui── UI (browser)
                                 └── serves the built UI on the same port
```

- `server/` — the server, a Rust crate. It keeps a session per test binary with its messages in
  order, answers a tree that asks what to run, and passes everything on to the UIs connected to
  it. It can be used as a library, and comes with a binary, `testscribe-frontend`.
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

`cargo run -- --listen 0.0.0.0:7357` listens for other machines too, and `--ui <dir>` serves a
UI built elsewhere.

## Working on the UI

`npm run dev` in `ui/` serves the UI with hot reload; it connects to a server on
`127.0.0.1:7357`. `npm run check` type-checks it.

## What the server tells the UI

The UI connects to `/ui` and is first sent everything known:
`{"kind": "snapshot", "sessions": [{"id", "hello", "events", "open"}]}`. A session is one test
binary; `hello` and `events` are its messages as it sent them (see the `protocol` module of the
`testscribe-detached` crate). After that, as it happens:

- `{"kind": "session_opened", "session": {...}}`
- `{"kind": "event", "session": <id>, "event": {...}}`
- `{"kind": "session_closed", "session": <id>}`, when the test binary has exited

The UI may send `{"kind": "clear"}` to forget the sessions that are over.

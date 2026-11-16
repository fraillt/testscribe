# testscribe-detached

An **experimental** connection from [testscribe](https://crates.io/crates/testscribe) tests to a
remote frontend that shows the run, and may choose what runs.

> ⚠️ **Experimental and unstable.** The API and the protocol may change in any release.

The tests still run under `cargo test`. When `TESTSCRIBE_FRONTEND` names a frontend, the test
binary connects to it and sends its test trees. Every standalone tree then asks the frontend what
to run before it starts, and reports each test as it goes. The terminal output stays what it is.

```toml
[dev-dependencies]
testscribe = { version = "0.2", features = ["detached"] }
```

```text
TESTSCRIBE_FRONTEND=127.0.0.1:7357 cargo test
```

Without the variable, or when nothing answers at that address, the tests run as usual.

`TESTSCRIBE_BATCH_ID` names the batch the results belong to, with any text. The frontend stores a
named batch, compares two of them, and remembers per batch what to run the next time: everything,
the tests selected in the UI, or the new and the failed ones. Without a name the results go to the
batch `default`, which the frontend keeps in memory only.

```text
TESTSCRIBE_FRONTEND=127.0.0.1:7357 TESTSCRIBE_BATCH_ID=before-refactoring cargo test
```

`TESTSCRIBE_RUN=all` or `TESTSCRIBE_RUN=new-and-failed` says what this run executes, whatever is
set in the frontend for the batch.

A frontend, a server with a web UI, lives in the
[`frontend`](https://github.com/fraillt/testscribe/tree/main/frontend) folder of the repository.
The messages are described in the `protocol` module of this crate.

## License

Licensed under the [MIT license](./LICENSE).

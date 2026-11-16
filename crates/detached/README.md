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

A frontend, a server with a web UI, lives in the
[`frontend`](https://github.com/fraillt/testscribe/tree/main/frontend) folder of the repository.
The messages are described in the `protocol` module of this crate.

## License

Licensed under the [MIT license](./LICENSE).

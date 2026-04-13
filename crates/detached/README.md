# testscribe-detached

An **experimental** remote-controlled test runner for the
[testscribe](https://crates.io/crates/testscribe) test framework. It lets a test tree be driven over
a connection by an external driver instead of the local `cargo test` harness.

> ⚠️ **Experimental and unstable.** The API may change or be removed in any release. Most users want
> the default [`testscribe-standalone`](https://crates.io/crates/testscribe-standalone) runner
> instead. See the [testscribe documentation](https://docs.rs/testscribe) to get started.

Enable it through the main crate's `detached` feature:

```toml
[dev-dependencies]
testscribe = { version = "0.1", features = ["detached"] }
```

## License

Licensed under the [MIT license](./LICENSE).

# testscribe-standalone

The default test runner for the [testscribe](https://crates.io/crates/testscribe) test framework.
It integrates testscribe's test trees with the standard test harness, so the whole tree runs under
plain `cargo test`.

> **Most users don't depend on this crate directly.** It is pulled in automatically by the main
> [`testscribe`](https://crates.io/crates/testscribe) crate through its default `standalone`
> feature. See the [testscribe documentation](https://docs.rs/testscribe) to get started.

You only need this crate directly if you are building a **custom test runner** and want to reuse the
standalone runner's building blocks (argument parsing, filtering, formatting). The core entry point
is [`run_test_tree`](https://docs.rs/testscribe-standalone/latest/testscribe_standalone/fn.run_test_tree.html):

```rust
use testscribe::standalone::args::Arguments;
use testscribe::standalone::run_all_sync;
use testscribe::CASES;
// In a `harness = false` integration test:
fn main() {
    run_all_sync(&CASES, Arguments::from_args())
        .unwrap()
        .exit_code()
}
```

See the `custom_test_runner` example in the
[main crate](https://github.com/fraillt/testscribe/tree/main/crates/testscribe) for a complete setup.

## License

Licensed under the [MIT license](./LICENSE).

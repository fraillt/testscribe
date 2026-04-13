# testscribe-core

Core library of the [testscribe](https://crates.io/crates/testscribe) test framework: the
test-tree data structure, execution logic, and assertion/reporting types.

> **You almost certainly don't want to depend on this crate directly.**
> It is an implementation detail re-exported by the main
> [`testscribe`](https://crates.io/crates/testscribe) crate, which is the entry point for users.
> See the [testscribe documentation](https://docs.rs/testscribe) to get started.

It is published as a separate crate mainly so that custom test runners (such as
[`testscribe-standalone`](https://crates.io/crates/testscribe-standalone)) can build on the same
building blocks.

## License

Licensed under the [MIT license](./LICENSE).

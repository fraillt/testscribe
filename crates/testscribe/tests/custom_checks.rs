// building the list item by item is the narrative being showcased, so `vec![1, 2, 5]` would
// defeat the purpose
#![allow(clippy::vec_init_then_push)]

use std::fmt::Debug;
// A custom check needs three pieces from `report`, imported explicitly here to make the
// anatomy obvious: `VerifyValue` (what `then!(x)` returns), `VerifyValueExposed` (its fields,
// reached via `::new`), and `VerifyOutcome` (the result you report). `use testscribe::prelude::*;`
// re-exports all three, so real test code rarely names them directly.
use testscribe::report::{VerifyOutcome, VerifyValue, VerifyValueExposed};
use testscribe::testscribe;

/// This trait defines a custom `sorted_ascending` check for `Vec<T>`.
///
/// Containment (`contains`) and equality (`eq`) already ship as built-in checks, so a good
/// custom check captures something the built-ins don't express — here, that a collection is
/// ordered. Implementing it as a check (instead of an inline `assert!`) means the intent shows
/// up verbatim in the test output.
pub trait CheckSortedAscending {
    fn sorted_ascending(self);
}

impl<T> CheckSortedAscending for VerifyValue<'_, Vec<T>>
where
    T: PartialOrd + Debug,
{
    fn sorted_ascending(self) {
        // `VerifyValue` deliberately exposes no public fields or methods, so that IDE
        // autocomplete on a `then!(...)` result only ever offers checks (`.eq`, `.contains`,
        // your custom ones), never internal plumbing. To reach the captured data from inside a
        // custom check, convert `self` into `VerifyValueExposed`, which exposes the same fields:
        //   - `actual_value`: the value under check, as `&T` (a reference, not owned)
        //   - `var_name`: the variable name / alias shown in the output
        //   - `reporter`: where the check outcome is recorded
        let this = VerifyValueExposed::new(self);
        let first_unordered = this
            .actual_value
            .windows(2)
            .position(|pair| pair[0] > pair[1]);
        this.reporter.set_outcome(
            format!("{} is sorted ascending", this.var_name),
            match first_unordered {
                None => VerifyOutcome::Success,
                Some(idx) => VerifyOutcome::Failure {
                    details: format!(
                        "out of order at index {idx}: {:?} > {:?}",
                        this.actual_value[idx],
                        this.actual_value[idx + 1]
                    ),
                },
            },
        );
    }
}

/// Running this test will give this outcome:
/// ```text
/// | 7.030μs|Given list is built in order
/// |       -|  Then list is sorted ascending
/// ```
#[testscribe(standalone)]
fn list_is_built_in_order() {
    let mut list = Vec::new();
    list.push(1);
    list.push(2);
    list.push(5);
    // `.sorted_ascending` comes from the custom `CheckSortedAscending` trait and is available
    // on the `VerifyValue` returned by the `then!` macro.
    then!(list).sorted_ascending();
}

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

/// A transfer the way a wallet stores it. Compared with `eq`, its whole `Debug` would land in
/// the test output: `Transfer { status: Confirmed, direction: Incoming, amount_micro: 50000000,
/// block: 101 }`. Nobody reads that, and it only gets worse with nesting.
pub struct Transfer {
    pub status: Status,
    pub direction: Direction,
    pub amount_micro: u64,
    pub block: u64,
}

#[derive(Debug)]
pub enum Status {
    Detected,
    Confirmed,
}

#[derive(Debug)]
pub enum Direction {
    Incoming,
    Outgoing,
}

/// A domain value told as one short sentence, the way the tests of this project word it.
///
/// It is a trait of the test code and not `Display`, for two reasons: the sentence a test
/// wants is rarely the one production code prints, and integration tests are a crate of their
/// own, where `Display` cannot be implemented for the types of the crate under test. A trait
/// defined in the tests can be.
pub trait Told {
    fn told(&self) -> String;
}

impl Told for Transfer {
    fn told(&self) -> String {
        format!(
            "{:?} {:?} {}.{:06} in block {}",
            self.status,
            self.direction,
            self.amount_micro / 1_000_000,
            self.amount_micro % 1_000_000,
            self.block
        )
    }
}

/// The check that goes with [`Told`]: written once, it works for every type that is told.
pub trait CheckReads {
    fn reads(self, expected: &str);
}

impl<T: Told> CheckReads for VerifyValue<'_, T> {
    fn reads(self, expected: &str) {
        let this = VerifyValueExposed::new(self);
        let told = this.actual_value.told();
        this.reporter.set_outcome(
            format!("{} reads {}", this.var_name, expected),
            if told == expected {
                VerifyOutcome::Success
            } else {
                VerifyOutcome::Failure {
                    details: format!("actual: {told}"),
                }
            },
        );
    }
}

/// Running this test will give this outcome:
/// ```text
/// | 8.120μs|Given deposit confirmed and withdrawal detected
/// |       -|  Then deposit reads Confirmed Incoming 50.000000 in block 101
/// |       -|  And withdrawal reads Detected Outgoing 12.500000 in block 103
/// ```
#[testscribe(standalone)]
fn deposit_confirmed_and_withdrawal_detected() {
    let transfers = [
        Transfer {
            status: Status::Confirmed,
            direction: Direction::Incoming,
            amount_micro: 50_000_000,
            block: 101,
        },
        Transfer {
            status: Status::Detected,
            direction: Direction::Outgoing,
            amount_micro: 12_500_000,
            block: 103,
        },
    ];
    // Items of a list are checked one by one, each under the name it has in the scenario.
    then!(transfers[0] => deposit).reads("Confirmed Incoming 50.000000 in block 101");
    then!(transfers[1] => withdrawal).reads("Detected Outgoing 12.500000 in block 103");
}

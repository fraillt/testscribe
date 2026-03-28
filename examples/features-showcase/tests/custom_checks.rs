use std::fmt::Debug;
use testscribe::processor::logger::VerifyOutcome;
use testscribe::report::{VerifyValue, VerifyValueExposed};
use testscribe::testscribe;

/// This trait defines a custom `contains` check for `Vec<T>`.
pub trait CheckContains<T> {
    fn contains(self, value: T)
    where
        T: PartialEq + Debug;
}

impl<T> CheckContains<T> for VerifyValue<'_, Vec<T>> {
    fn contains(self, value: T)
    where
        T: PartialEq + Debug,
    {
        // All fields of `VerifyValue` are private, so we convert `self` to
        // `VerifyValueExposed`, which provides public access to the same data.
        // `VerifyValue` fields are private to keep IDE suggestions clean,
        // because users are not expected to access those internals directly.
        let this = VerifyValueExposed::new(self);
        this.reporter.set_outcome(
            format!("{} contains {:?}", this.var_name, value),
            if this.actual_value.contains(&value) {
                VerifyOutcome::Success
            } else {
                VerifyOutcome::Failure {
                    details: format!("actual values: {:?}", this.actual_value),
                }
            },
        );
    }
}

/// Running this test will give this outcome:
/// ```text
/// | 7.030μs|Given item to list is added
/// |       -|  Then list contains 3
/// ```
#[testscribe(standalone)]
#[test]
fn item_to_list_is_added() {
    let mut list = Vec::new();
    list.push(3);
    // `.contains` comes from the custom `CheckContains` trait and is available
    // on the `VerifyValue` returned by the `then!` macro.
    then!(list).contains(3);
}

// For demonstration purposes some test function doesn't have assertions, so we suppress the unused macro warning.
#![allow(unused_macros)]

mod utils;

use testscribe::{CASES, test_args::Given, testscribe};
use utils::tree::create_and_verify_tt;

mod mod1 {
    use testscribe::{test_args::Given, testscribe};

    pub mod mod2 {
        use testscribe::{test_args::Given, testscribe};

        #[testscribe]
        pub fn root_test() {}

        #[testscribe]
        fn defined_in_same_module_as_root_test(_: Given<RootTest>) {}

        #[testscribe]
        fn back_and_forth(_: Given<super::mod2::RootTest>) {}
    }

    #[testscribe]
    fn relative_path(_: Given<mod2::RootTest>) {}

    #[testscribe]
    fn absolute_path(_: Given<super::AbsolutePath>) {}

    #[testscribe]
    fn using_crate(_: Given<crate::mod1::mod2::RootTest>) {}
}

#[testscribe]
pub fn absolute_path(_: Given<mod1::mod2::RootTest>) {}

#[test]
fn resolve_parent_from_other_submodules() {
    create_and_verify_tt(&CASES, false).unwrap();
}

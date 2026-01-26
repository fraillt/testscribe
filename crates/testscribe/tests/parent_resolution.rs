mod utils;

use testscribe::{CASES, test_args::Given, testscribe};
use utils::tree::create_and_verify_tt;

mod mod1 {
    use testscribe::{test_args::Given, testscribe};

    pub mod mod2 {
        use testscribe::{test_args::Given, testscribe};

        #[testscribe]
        pub fn root_test() {
            then!("");
        }

        #[testscribe]
        fn defined_in_same_module_as_root_test(_: Given<RootTest>) {
            then!("");
        }

        #[testscribe]
        fn back_and_forth(_: Given<super::mod2::RootTest>) {
            then!("");
        }
    }

    #[testscribe]
    fn relative_path(_: Given<mod2::RootTest>) {
        then!("");
    }

    #[testscribe]
    fn absolute_path(_: Given<super::AbsolutePath>) {
        then!("");
    }

    #[testscribe]
    fn using_crate(_: Given<crate::mod1::mod2::RootTest>) {
        then!("");
    }
}

#[testscribe]
pub fn absolute_path(_: Given<mod1::mod2::RootTest>) {
    then!("");
}

#[test]
fn resolve_parent_from_other_submodules() {
    create_and_verify_tt(&CASES, false).unwrap();
}

#![allow(dead_code)]

use testscribe::test_case::FqFnName;
pub mod tree;

pub fn create_fq_name(fq_name: &'static str) -> FqFnName<'static> {
    match fq_name.rsplit_once("::") {
        Some((path, name)) => FqFnName { path, name },
        None => FqFnName {
            path: "",
            name: fq_name,
        },
    }
}

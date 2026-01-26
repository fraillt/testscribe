#![allow(dead_code)]

use testscribe::test_case::FqFnName;
pub mod tree;

pub fn create_fq_name(fq_name: &'static str) -> FqFnName<'static> {
    let (path, name) = fq_name.rsplit_once("::").unwrap();
    FqFnName::new(path, name)
}

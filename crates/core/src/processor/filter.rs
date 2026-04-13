use std::collections::HashSet;

use crate::{
    processor::logger::TestRunInfo,
    test_case::{FqFnName, TestCase},
};

pub trait Filter {
    fn should_run(&self, test: &'static TestCase, info: &TestRunInfo) -> bool;
}

pub struct NoFilter;

impl Filter for NoFilter {
    fn should_run(&self, _test: &'static TestCase, _info: &TestRunInfo) -> bool {
        true
    }
}

pub struct FilterByPath {
    pub paths: Vec<HashSet<FqFnName<'static>>>,
}

impl Filter for FilterByPath {
    fn should_run(&self, test: &'static TestCase, info: &TestRunInfo) -> bool {
        for (curr, list) in info.path.iter().chain(Some(&test.name)).zip(&self.paths) {
            if !list.contains(curr) {
                return false;
            }
        }
        self.paths.len() > info.path.len()
    }
}

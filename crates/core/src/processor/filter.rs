use crate::{processor::logger::TestRunInfo, test_case::TestCase};

pub trait Filter {
    fn should_run(&self, test: &'static TestCase, info: &TestRunInfo) -> bool;
}

pub struct NoFilter;

impl Filter for NoFilter {
    fn should_run(&self, _test: &'static TestCase, _info: &TestRunInfo) -> bool {
        true
    }
}

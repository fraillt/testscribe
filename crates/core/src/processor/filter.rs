use crate::processor::logger::TestRunInfo;

pub trait Filter {
    fn should_run(&self, test: &TestRunInfo) -> bool;
}

pub struct NoFilter;

impl Filter for NoFilter {
    fn should_run(&self, _test: &TestRunInfo) -> bool {
        true
    }
}

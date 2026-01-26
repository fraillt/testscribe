mod summary_formatter;
mod test_formatter;

use std::time::Instant;

use testscribe_core::{
    processor::logger::{Logger, TestStatusUpdate},
    test_case::FqFnName,
};

pub use crate::logger::test_formatter::TestFormatter;

// re-export
pub use summary_formatter::TestSummary;

#[derive(Debug, Clone)]
pub struct Failure {
    pub param_index: Option<usize>,
    pub message: String,
    pub line_nr: u32,
    pub file: &'static str,
    pub details: String,
}

pub struct FailedTest {
    pub name: FqFnName<'static>,
    pub failures: Vec<Failure>,
}

pub struct TestLogger<'a> {
    pub formatter: TestFormatter<'a>,
    pub created_at: Instant,
}

impl Logger for TestLogger<'_> {
    fn log(&mut self, update: TestStatusUpdate) {
        self.formatter
            .replay_event(update, self.created_at.elapsed())
    }
}

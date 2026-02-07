mod check_report;
mod verify_object;

/// Exposes basic functionality in order to verify values or statements
pub mod basic;

use std::rc::Rc;
use std::{cell::RefCell, time::Instant};

pub use check_report::{CheckReporter, ParamCheckReporter};
pub use verify_object::{VerifyStatement, VerifyStatementExposed, VerifyValue, VerifyValueExposed};

use crate::{
    processor::logger::{Logger, TestStatusUpdate, TestUpdate},
    test_case::TestCase,
};

pub struct TestReport {
    test: &'static TestCase,
    logger: Rc<RefCell<&'static mut dyn Logger>>,
    started_at: Instant,
}

impl TestReport {
    pub fn new(
        test: &'static TestCase,
        logger: Rc<RefCell<&'static mut dyn Logger>>,
        started_at: Instant,
    ) -> TestReport {
        Self {
            test,
            logger,
            started_at,
        }
    }

    fn update(&self, info: TestUpdate) {
        let mut logger = self.logger.borrow_mut();
        logger.log(
            self.test,
            TestStatusUpdate::Updated { info },
            self.started_at.elapsed(),
        );
    }
}

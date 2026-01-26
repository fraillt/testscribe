mod check_report;
mod verify_object;

/// Exposes basic functionality in order to verify values or statements
pub mod basic;

use std::cell::RefCell;
use std::rc::Rc;

pub use check_report::{CheckReporter, ParamCheckReporter};
pub use verify_object::{VerifyStatement, VerifyStatementExposed, VerifyValue, VerifyValueExposed};

use crate::processor::logger::{Logger, TestStatusUpdate, TestUpdate};

pub struct TestReport {
    logger: Rc<RefCell<&'static mut dyn Logger>>,
}

impl TestReport {
    pub fn new(logger: Rc<RefCell<&'static mut dyn Logger>>) -> TestReport {
        Self { logger }
    }

    fn update(&self, info: TestUpdate) {
        let mut logger = self.logger.borrow_mut();
        logger.log(TestStatusUpdate::Updated { info });
    }
}

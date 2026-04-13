//! Reporting and assertion types behind the `then!` macro.
//!
//! The `then!` macro (generated inside every `#[testscribe]` test) creates a [`VerifyValue`]
//! or [`VerifyStatement`]. Checks on them come from extension traits — the built-in ones live
//! in the [`basic`] module, and custom domain-specific checks can be added via
//! [`VerifyValueExposed`] / [`VerifyStatementExposed`].

mod check_report;
mod verify_object;

/// Exposes basic functionality in order to verify values or statements
pub mod basic;

use std::rc::Rc;
use std::{cell::RefCell, time::Instant};

pub use check_report::{CheckReporter, ParamCheckReporter, VerifyOutcome};
pub use verify_object::{VerifyStatement, VerifyStatementExposed, VerifyValue, VerifyValueExposed};

use crate::{
    processor::logger::{Logger, TestStatusUpdate, TestUpdate},
    test_case::TestCase,
};

/// Live report of the currently executing test; check outcomes are streamed through it.
///
/// Created by the framework and passed into every test function as a hidden first argument,
/// where the generated `then!` macro picks it up. You should never need to construct or
/// touch it directly.
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

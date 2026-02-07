use std::mem::take;
use std::sync::{Arc, Mutex};

use testscribe_core::processor::TestsRunner;
use testscribe_core::processor::filter::Filter;
use testscribe_core::processor::logger::Logger;
use testscribe_core::tests_tree::TestsTree;

use crate::logger::summary::{ExecutionSummary, TestsTreeLogger};
use crate::panic_hook::PanicHandler;

pub async fn run_test_tree<P>(
    tree: TestsTree,
    filter: &dyn Filter,
    printer: &mut P,
    enable_panic_hook: bool,
) -> ExecutionSummary
where
    P: Logger,
{
    let mut logger = TestsTreeLogger::new(printer);
    if enable_panic_hook {
        let panics = Arc::new(Mutex::new(Vec::new()));
        {
            let panics_cloned = panics.clone();
            let _handler = PanicHandler::attach_panic_hook(move |details| {
                panics_cloned.lock().unwrap().push(details);
            });
            TestsRunner::run_tests(&tree, filter, &mut logger).await;
        }
        let mut summary = logger.into_summary();
        summary.panics = take(panics.lock().unwrap().as_mut());
        summary
    } else {
        TestsRunner::run_tests(&tree, filter, &mut logger).await;
        logger.into_summary()
    }
}

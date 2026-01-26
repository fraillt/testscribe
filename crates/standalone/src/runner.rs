use std::env::Args;
use std::io::Write;
use std::mem::take;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use thiserror::Error;

use testscribe_core::processor::{TestsRunner, filter::NoFilter};
use testscribe_core::test_case::{FqFnName, TestCase};
use testscribe_core::tests_tree::{BuildDagError, create_test_trees};

use crate::logger::{TestFormatter, TestLogger, TestSummary};
use crate::panic_hook::PanicHandler;

#[derive(Error, Debug)]
pub enum TestFailError {
    #[error("{0} failed.")]
    TestFailed(&'static str),
    #[error(transparent)]
    BuildDagErr(#[from] BuildDagError),
}

pub struct Config {
    pub enable_panic_collector: bool,
    pub immediate_output: bool,
}

impl Config {
    pub fn from_args(args: Args) -> Self {
        let mut enable_panic_collector = false;
        let mut immediate_output = false;

        for arg in args {
            // if test runner runs exactly this test,
            // enable panic collector for extra info
            if arg.as_str() == "--exact" {
                enable_panic_collector = true
            }
            // if there's no std output is not captured, then immediatelly
            // log test updates as the test progresses
            if arg.as_str() == "--nocapture" {
                immediate_output = true;
            }
        }

        Self {
            enable_panic_collector,
            immediate_output,
        }
    }
}

pub async fn run_test(
    test_cases: &'static [TestCase],
    module_path: &'static str,
    test_name: &'static str,
    is_async_runtime: bool,
    cfg: Config,
) -> Result<(), TestFailError> {
    let mut dags = create_test_trees(test_cases)?;
    let dag = dags.remove(&FqFnName::new(module_path, test_name)).unwrap();
    dag.verify(is_async_runtime)?;

    let mut stdout = std::io::stdout();
    let mut vecout = Vec::new();
    let output: &mut dyn Write = if cfg.immediate_output {
        &mut stdout
    } else {
        &mut vecout
    };

    let mut logger = TestLogger {
        formatter: TestFormatter::new(&dag, output),
        created_at: Instant::now(),
    };

    let summary = if cfg.enable_panic_collector {
        let panics = Arc::new(Mutex::new(Vec::new()));
        {
            let panics_cloned = panics.clone();
            let _handler = PanicHandler::attach_panic_hook(move |details| {
                panics_cloned.lock().unwrap().push(details);
            });

            TestsRunner::run_tests(&dag, &NoFilter, &mut logger).await;
        }
        TestSummary::new(
            logger.formatter.failed_tests,
            take(panics.lock().unwrap().as_mut()),
        )
    } else {
        TestsRunner::run_tests(&dag, &NoFilter, &mut logger).await;
        TestSummary::new(logger.formatter.failed_tests, Default::default())
    };
    summary.print_summary(output);
    if !cfg.immediate_output {
        print!("{}", String::from_utf8(vecout).unwrap());
    }
    if summary.is_success() {
        Ok(())
    } else {
        Err(TestFailError::TestFailed(test_name))
    }
}

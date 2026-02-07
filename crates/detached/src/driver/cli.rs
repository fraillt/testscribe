use std::collections::{BTreeMap, HashMap};
use std::mem::take;
use std::process::ExitCode;
use std::time::Duration;

use futures::StreamExt;
use testscribe_standalone::logger::printer::TestFormatter;
use testscribe_standalone::logger::summary::TestsTreeLogger;

use crate::runtime::interface::{CommandSender, Frontend, StatusReceiver};
use crate::runtime::messages::{
    CommandMsg, RunTestTree, StatusMsg, TestTreeFilter, TestTreeStatusUpdate,
};
use testscribe_core::processor::logger::{Logger, TestStatusUpdate};
use testscribe_core::test_case::FqFnName;
use testscribe_core::tests_tree::TestsTree;

pub struct CliFrontend {
    pub concurrency: usize,
}

impl Frontend for CliFrontend {
    async fn start(
        self,
        trees: BTreeMap<FqFnName<'static>, TestsTree>,
        command_sender: CommandSender,
        mut status_receiver: StatusReceiver,
    ) -> ExitCode {
        let mut running = HashMap::new();
        let mut trees_iter = trees.into_iter();
        let mut gen_tree_id = 0;
        for (root_test, _) in trees_iter.by_ref().take(self.concurrency) {
            gen_tree_id += 1;
            running.insert(gen_tree_id, PrintTestOutcome::new());
            command_sender
                .unbounded_send(CommandMsg::RunTestTrees {
                    trees: vec![RunTestTree {
                        id: gen_tree_id,
                        root_test: root_test.into(),
                        filter: TestTreeFilter::RunAll,
                    }],
                })
                .expect("Backend must be alive");
        }
        let mut failures_count = 0;
        while let Some(status) = status_receiver.next().await {
            match status {
                StatusMsg::TestTreeStatus { tree_id, update } => match update {
                    TestTreeStatusUpdate::Started => {}
                    TestTreeStatusUpdate::Finished => {
                        failures_count += running.remove(&tree_id).unwrap().failed_tests;
                        if self.concurrency > running.len() {
                            if let Some((root_test, _)) = trees_iter.next() {
                                gen_tree_id += 1;
                                running.insert(gen_tree_id, PrintTestOutcome::new());
                                command_sender
                                    .unbounded_send(CommandMsg::RunTestTrees {
                                        trees: vec![RunTestTree {
                                            id: gen_tree_id,
                                            root_test: root_test.into(),
                                            filter: TestTreeFilter::RunAll,
                                        }],
                                    })
                                    .expect("Backend must be alive");
                            }
                        }
                        if running.is_empty() {
                            if failures_count > 0 {
                                eprintln!("number of failed tests: {failures_count}");
                                return ExitCode::FAILURE;
                            } else {
                                return ExitCode::SUCCESS;
                            }
                        }
                    }
                },
                StatusMsg::TestStatus {
                    tree_id,
                    test,
                    update,
                    elapsed,
                } => {
                    let printer = running.get_mut(&tree_id).unwrap();
                    printer.log(test, update, elapsed);
                }
                StatusMsg::Panic { details: _ } => todo!(),
                StatusMsg::InvalidCommandError { message: _ } => todo!(),
            }
        }
        ExitCode::SUCCESS
    }
}

struct PrintTestOutcome {
    events: Vec<(TestStatusUpdate, Duration)>,
    failed_tests: usize,
}

impl PrintTestOutcome {
    fn new() -> Self {
        Self {
            events: Vec::new(),
            failed_tests: 0,
        }
    }
}

impl Logger for PrintTestOutcome {
    fn log(
        &mut self,
        test: &'static testscribe_core::test_case::TestCase,
        update: TestStatusUpdate,
        elapsed: Duration,
    ) {
        let is_finished = matches!(&update, TestStatusUpdate::Finished { panic_message: _ });
        self.events.push((update, elapsed));
        if is_finished {
            let mut outcome = std::io::stdout().lock();
            let mut printer = TestFormatter::new(&mut outcome);
            let mut logger = TestsTreeLogger::new(&mut printer);
            for (update, elapsed) in take(&mut self.events) {
                logger.log(test, update, elapsed);
            }
            let summary = logger.into_summary();
            self.failed_tests += summary.failed.len();
            printer.print_failures(&summary.failed);
            printer.print_panics(&summary.panics);
        }
    }
}

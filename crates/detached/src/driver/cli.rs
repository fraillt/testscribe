use std::collections::{BTreeMap, HashMap};
use std::mem::take;
use std::process::ExitCode;
use std::time::Duration;

use futures::StreamExt;

use crate::runtime::interface::{CommandSender, Frontend, StatusReceiver};
use crate::runtime::messages::{
    CommandMsg, RunTestTree, StatusMsg, TestTreeFilter, TestTreeStatusUpdate,
};
use testscribe_core::processor::logger::TestStatusUpdate;
use testscribe_core::test_case::FqFnName;
use testscribe_core::tests_tree::TestsTree;
use testscribe_standalone::logger::{TestFormatter, TestSummary};

pub struct CliFrontend {
    pub concurrency: usize,
}

impl Frontend for CliFrontend {
    async fn start(
        self,
        dags: BTreeMap<FqFnName<'static>, TestsTree>,
        command_sender: CommandSender,
        mut status_receiver: StatusReceiver,
    ) -> ExitCode {
        let mut running = HashMap::new();
        let mut dags_iter = dags.into_iter();
        let mut gen_tree_id = 0;
        for (root_test, tree_info) in dags_iter.by_ref().take(self.concurrency) {
            gen_tree_id += 1;
            running.insert(gen_tree_id, PrintTestOutcome::new(tree_info));
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
                        failures_count += running.remove(&tree_id).unwrap().failures_count;
                        if self.concurrency > running.len() {
                            if let Some((root_test, tree_info)) = dags_iter.next() {
                                gen_tree_id += 1;
                                running.insert(gen_tree_id, PrintTestOutcome::new(tree_info));
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
                    update,
                    elapsed,
                } => {
                    let printer = running.get_mut(&tree_id).unwrap();
                    printer.log(update, elapsed);
                }
                StatusMsg::Panic { details: _ } => todo!(),
                StatusMsg::InvalidCommandError { message: _ } => todo!(),
            }
        }
        ExitCode::SUCCESS
    }
}

struct PrintTestOutcome {
    tree: TestsTree,
    events: Vec<(TestStatusUpdate, Duration)>,
    failures_count: usize,
}

impl PrintTestOutcome {
    fn new(tree: TestsTree) -> Self {
        Self {
            tree,
            events: Vec::new(),
            failures_count: 0,
        }
    }

    fn log(&mut self, update: TestStatusUpdate, elapsed: Duration) {
        let is_finished = matches!(&update, TestStatusUpdate::Finished { panic_message: _ });
        self.events.push((update, elapsed));
        if is_finished {
            let mut outcome = std::io::stdout().lock();
            let mut formatter = TestFormatter::new(&self.tree, &mut outcome);
            for (update, elapsed) in take(&mut self.events) {
                formatter.replay_event(update, elapsed);
            }
            self.failures_count += formatter.failed_tests.len();
            let summary = TestSummary::new(formatter.failed_tests, Default::default());
            summary.print_summary(&mut outcome);
        }
    }
}

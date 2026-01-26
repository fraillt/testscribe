pub mod interface;
pub mod messages;
pub mod status_sender;

use std::collections::BTreeMap;
use std::future::Future;
use std::process::ExitCode;
use std::thread::spawn;
use std::time::Instant;

use futures::channel::mpsc::{UnboundedSender, unbounded};
use futures::executor::block_on;
use futures::future::join_all;
use futures::{FutureExt, StreamExt, pin_mut, select};
use status_sender::{StatusSender, TestStatusUpdateMsg};
use testscribe_standalone::panic_hook::PanicHandler;
use tokio::runtime::Builder;
use tokio::task::LocalSet;

use crate::runtime::interface::Frontend;
use crate::runtime::messages::{
    CommandMsg, RunTestTree, StatusMsg, TestTreeFilter, TestTreeStatusUpdate,
};
use testscribe_core::processor::logger::TestRunInfo;
use testscribe_core::processor::{TestsRunner, filter::Filter};
use testscribe_core::test_case::FqFnName;
use testscribe_core::tests_tree::TestsTree;

impl Filter for TestTreeFilter {
    fn should_run(&self, test: &TestRunInfo) -> bool {
        match &self {
            TestTreeFilter::RunAll => true,
            TestTreeFilter::RunPaths { paths } => paths[test.depth]
                .iter()
                .any(|t| t.as_fq_fn_name() == test.name),
        }
    }
}

impl From<(u64, TestStatusUpdateMsg)> for StatusMsg {
    fn from((tree_id, info): (u64, TestStatusUpdateMsg)) -> Self {
        Self::TestStatus {
            tree_id: tree_id,
            update: info.update,
            elapsed: info.elapsed,
        }
    }
}

pub trait DagsRuntime {
    /// spawn a task where tests will be running
    fn spawn<F, Fut>(&mut self, f: F)
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: Future<Output = ()> + 'static;
}

#[derive(Default)]
pub struct TokioRuntime {}

impl DagsRuntime for TokioRuntime {
    fn spawn<F, Fut>(&mut self, f: F)
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: Future<Output = ()> + 'static,
    {
        spawn(move || {
            let local_set = LocalSet::new();
            local_set.spawn_local(f());
            let rt = Builder::new_current_thread().enable_all().build().unwrap();
            rt.block_on(local_set);
        });
    }
}

#[derive(Default)]
pub struct SyncRuntime {}

impl DagsRuntime for SyncRuntime {
    fn spawn<F, Fut>(&mut self, f: F)
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: Future<Output = ()> + 'static,
    {
        spawn(|| {
            block_on(async move {
                f().await;
            });
        });
    }
}

struct TestTreeRunner {
    tree_id: u64,
    tree: TestsTree,
    filter_mode: TestTreeFilter,
    sender: UnboundedSender<StatusMsg>,
}

impl TestTreeRunner {
    async fn run(self) {
        let mut processor = StatusSender::new(self.tree_id, Instant::now(), self.sender.clone());
        self.sender
            .unbounded_send(StatusMsg::TestTreeStatus {
                tree_id: self.tree_id,
                update: TestTreeStatusUpdate::Started,
            })
            .unwrap();
        TestsRunner::run_tests(&self.tree, &self.filter_mode, &mut processor).await;
        self.sender
            .unbounded_send(StatusMsg::TestTreeStatus {
                tree_id: self.tree_id,
                update: TestTreeStatusUpdate::Finished,
            })
            .unwrap();
    }
}

pub async fn start_backend<Fr, Rt>(
    dags: BTreeMap<FqFnName<'static>, TestsTree>,
    frontend: Fr,
    runtime: Rt,
) -> ExitCode
where
    Fr: Frontend + Send + 'static,
    Rt: DagsRuntime,
{
    let (command_sender, mut command_receiver) = unbounded();
    let (status_sender, status_receiver) = unbounded();
    if dags.is_empty() {
        return ExitCode::SUCCESS;
    }
    let frontend_fut = frontend
        .start(dags.clone(), command_sender, status_receiver)
        .fuse();
    let mut command_fut = command_receiver.next();
    pin_mut!(frontend_fut);
    let mut processor = CommandProcessor {
        dags,
        status_sender,
        runtime,
        panic_handler: None,
    };
    eprintln!("********** start backend loop:");
    loop {
        select! {
            res = frontend_fut => {
                eprintln!("********** frontend finished");
                return res;
            },
            cmd = command_fut => {
                eprintln!("******* process command: {cmd:?}");
                if let Some(cmd) = cmd {
                    processor.process_cmd(cmd);
                }
                command_fut = command_receiver.next()
            }
        }
    }
}

struct CommandProcessor<R> {
    dags: BTreeMap<FqFnName<'static>, TestsTree>,
    status_sender: UnboundedSender<StatusMsg>,
    runtime: R,
    panic_handler: Option<PanicHandler>,
}

impl<R> Drop for CommandProcessor<R> {
    fn drop(&mut self) {
        self.panic_handler.take();
    }
}

impl<R> CommandProcessor<R>
where
    R: DagsRuntime,
{
    fn process_cmd(&mut self, cmd: CommandMsg) {
        match cmd {
            CommandMsg::RunTestTrees { trees: dags } => {
                self.start_dags(dags);
            }
            CommandMsg::EnablePanicsCollector => {
                self.panic_handler.get_or_insert_with(|| {
                    let sender = self.status_sender.clone();
                    PanicHandler::attach_panic_hook(move |details| {
                        sender.unbounded_send(StatusMsg::Panic { details }).unwrap();
                    })
                });
            }
            CommandMsg::DisablePanicsCollector => {
                self.panic_handler.take();
            }
        }
    }

    fn start_dags(&mut self, list: Vec<RunTestTree>) {
        let runners: Vec<_> = list
            .into_iter()
            .filter_map(|info| {
                self.dags
                    .get(&info.root_test.as_fq_fn_name())
                    .map(|tree| TestTreeRunner {
                        tree_id: info.id,
                        filter_mode: info.filter,
                        tree: tree.clone(),
                        sender: self.status_sender.clone(),
                    })
            })
            .collect();
        if !runners.is_empty() {
            let is_async = runners[0].tree.node.test_fn.is_async();
            if let Some(different) = runners
                .iter()
                .find(|runner| runner.tree.node.test_fn.is_async() != is_async)
            {
                self.status_sender
                    .unbounded_send(StatusMsg::InvalidCommandError {
                        message: format!(
                            "All test must be same syncness, but `{}` and `{}` are different.",
                            runners[0].tree.node.name, different.tree.node.name
                        ),
                    })
                    .unwrap()
            } else {
                self.runtime.spawn(|| async {
                    join_all(runners.into_iter().map(|r| r.run())).await;
                });
            }
        } else {
            self.status_sender
                .unbounded_send(StatusMsg::InvalidCommandError {
                    message: "No tests to run".to_string(),
                })
                .unwrap()
        }
    }
}

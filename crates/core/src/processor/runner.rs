use std::time::Instant;

use super::run_state::RunState;
use crate::processor::filter::Filter;
use crate::processor::logger::{Logger, ParamInfo, TestRunInfo};
use crate::test_case::{FqFnName, TestParams, Value};
use crate::tests_tree::{AugmentedTestsTree, TestsTree};

pub struct TestsRunner {}

impl TestsRunner {
    pub async fn run_tests(tree: &TestsTree, filter: &dyn Filter, logger: &mut dyn Logger) {
        let tree = AugmentedTestsTree::new(tree, |node| {
            node.params
                .as_ref()
                .map(|p| (p.params)())
                .unwrap_or_else(TestParams::new_empty)
        });
        let mut state = Vec::new();
        let mut state_idx = 0;
        state.push(TestState::new(vec![], &tree, RunState::init()));

        #[derive(Clone, Copy)]
        enum ProgressAction {
            ProcessChain,
            FindBranch,
        }

        let started_at = Instant::now();

        let mut action = ProgressAction::ProcessChain;

        while !state.is_empty() {
            let exec_state_len = state.len();
            let curr = &mut state[state_idx];
            if let (ProgressAction::ProcessChain, true) = (action, curr.state.process.is_some()) {
                let process = curr.state.process.take().unwrap();
                let new_run = match process {
                    TestProcess::Process { run_state } => {
                        let (info, param_value) = curr.prepare_test_run();

                        curr.state.run_count += 1;
                        run_state
                            .run_test(
                                filter,
                                logger,
                                curr.tree.node.test,
                                started_at,
                                info,
                                param_value,
                            )
                            .await
                    }
                    TestProcess::AlreadyProcessed { outcome } => outcome,
                };

                let child_run = if let Some(clone_fns) = curr.tree.node.test.clone.as_ref() {
                    let has_continuation = exec_state_len > state_idx + 1
                        || curr.state.childs_idx < curr.tree.childs.len();
                    if has_continuation {
                        let clone = new_run.clone_state(clone_fns).await;
                        curr.state.process =
                            Some(TestProcess::AlreadyProcessed { outcome: new_run });
                        Some(clone)
                    } else {
                        None
                    }
                } else {
                    Some(new_run)
                };

                if exec_state_len > state_idx + 1 {
                    state_idx += 1;
                    state[state_idx].state.process = Some(TestProcess::Process {
                        run_state: child_run.expect("a continuation exists"),
                    });
                } else if curr.state.childs_idx < curr.tree.childs.len() {
                    let next = &curr.tree.childs[curr.state.childs_idx];
                    state_idx += 1;
                    let path = curr
                        .path
                        .iter()
                        .copied()
                        .chain(std::iter::once(curr.tree.node.test.name))
                        .collect();
                    state.push(TestState::new(
                        path,
                        next,
                        child_run.expect("a continuation exists"),
                    ));
                } else {
                    action = ProgressAction::FindBranch;
                }
            } else {
                if let ProgressAction::FindBranch = action {
                    curr.advance();
                    if !curr.is_processed() {
                        action = ProgressAction::ProcessChain;
                        state_idx += 1;
                    } else {
                        state.pop();
                    }
                }

                if state_idx > 0 {
                    state_idx -= 1;
                } else if !state.is_empty() {
                    let root = &mut state[0];
                    root.state.process = Some(TestProcess::Process {
                        run_state: RunState::init(),
                    })
                }
            }
        }
    }
}

enum TestProcess {
    Process { run_state: RunState },
    AlreadyProcessed { outcome: RunState },
}

struct State {
    process: Option<TestProcess>,
    args_idx: usize,
    childs_idx: usize,
    run_count: usize,
}

struct TestState<'a> {
    tree: &'a AugmentedTestsTree<TestParams>,
    state: State,
    path: Vec<FqFnName<'static>>,
}

impl<'a> TestState<'a> {
    fn new(
        path: Vec<FqFnName<'static>>,
        tree: &'a AugmentedTestsTree<TestParams>,
        run_state: RunState,
    ) -> Self {
        Self {
            tree,
            path,
            state: State {
                process: Some(TestProcess::Process { run_state }),
                args_idx: 0,
                childs_idx: 0,
                run_count: 0,
            },
        }
    }

    fn prepare_test_run(&self) -> (TestRunInfo, Option<Value>) {
        let (param_info, param_value) = if self.state.args_idx < self.tree.node.extra.len() {
            let arg = self.tree.node.extra.get(self.state.args_idx);
            (
                Some(ParamInfo {
                    headers: arg.header,
                    display_str: arg.display_str,
                }),
                Some(arg.value),
            )
        } else {
            (None, None)
        };
        (
            TestRunInfo {
                path: self.path.clone(),
                run_count: self.state.run_count,
                param_info,
            },
            param_value,
        )
    }

    fn is_processed(&self) -> bool {
        self.tree.childs.len() == self.state.childs_idx
            && self.tree.node.extra.len() == self.state.args_idx
    }

    fn advance(&mut self) {
        if self.state.childs_idx < self.tree.childs.len() {
            self.state.childs_idx += 1;
            if self.state.childs_idx == self.tree.childs.len()
                && self.state.args_idx < self.tree.node.extra.len()
            {
                self.state.args_idx += 1;
                self.state.run_count = 0;
                self.state.process = None;
                if self.state.args_idx < self.tree.node.extra.len() {
                    self.state.childs_idx = 0;
                }
            }
        } else if self.state.args_idx < self.tree.node.extra.len() {
            self.state.args_idx += 1;
            self.state.run_count = 0;
            self.state.process = None;
            self.state.childs_idx = 0;
        }
    }
}

#[cfg(test)]
pub mod tests {

    use std::time::Duration;

    use futures::executor::block_on;

    use super::*;
    use crate::processor::filter::NoFilter;
    use crate::processor::logger::TestStatusUpdate;
    use crate::test_args::ParamDisplay;
    use crate::test_case::{
        CloneFn, CloneFns, FqFnName, ParamsFn, ParentFn, TestCase, TestFn, TestParams,
    };
    use crate::tests_tree::create_test_trees;

    struct TestCaseBuilder {
        fn_name: FqFnName<'static>,
        is_cloneable: bool,
        parent: Option<ParentFn>,
        params: Option<fn() -> TestParams>,
    }

    impl TestCaseBuilder {
        const fn new(fn_name: &'static str) -> Self {
            Self {
                fn_name: FqFnName::new("", fn_name),
                is_cloneable: false,
                parent: None,
                params: None,
            }
        }

        const fn set_cloneable(mut self) -> Self {
            self.is_cloneable = true;
            self
        }

        const fn depends_on(mut self, get_name: fn() -> FqFnName<'static>) -> Self {
            self.parent = Some(ParentFn { get_name });
            self
        }

        const fn with_params(mut self, params: fn() -> TestParams) -> Self {
            self.params = Some(params);
            self
        }

        fn build(self) -> TestCase {
            TestCase {
                name: self.fn_name,
                tags: &[],
                filename: "",
                line_nr: 0,
                test_fn: TestFn::SyncFn(|_, v, _e, _p| v),
                parent: self.parent,
                env: None,
                params: self.params.map(|params| ParamsFn { params }),
                clone: if self.is_cloneable {
                    Some(CloneFns {
                        state: CloneFn::new_sync::<()>(),
                        env: CloneFn::new_sync::<()>(),
                    })
                } else {
                    None
                },
            }
        }
    }

    fn action(path: &'static str, run_count: usize) -> TestRunInfo {
        TestRunInfo {
            path: path
                .split(".")
                .map(|name| FqFnName::new("", name))
                .collect(),
            run_count,
            param_info: None,
        }
    }

    fn action_with_param(path: &'static str, run_count: usize, param: &str) -> TestRunInfo {
        TestRunInfo {
            path: path
                .split(".")
                .map(|name| FqFnName::new("", name))
                .collect(),
            run_count,
            param_info: Some(ParamInfo {
                headers: vec!["name"],
                display_str: vec![param.to_string()],
            }),
        }
    }

    fn assert_tree(cases: Vec<TestCaseBuilder>, expected_runs: Vec<TestRunInfo>) {
        let cases = cases
            .into_iter()
            .map(TestCaseBuilder::build)
            .collect::<Vec<_>>();
        let leaked: &'static [TestCase] = Box::leak(cases.into_boxed_slice());
        let mut trees = create_test_trees(leaked).into_iter();
        let tree = trees.next().unwrap();
        assert!(trees.next().is_none());
        tree.verify(false).unwrap();

        let mut logger = LogActions::default();
        block_on(TestsRunner::run_tests(&tree, &NoFilter, &mut logger));
        assert_eq!(logger.actions.len(), expected_runs.len(), "number of runs");

        for (index, (actual, expected)) in logger.actions.into_iter().zip(expected_runs).enumerate()
        {
            assert_eq!(actual.path, expected.path, "run index: {index}");
            assert_eq!(actual.run_count, expected.run_count, "run index: {index}");
            assert_eq!(
                actual.param_info.is_some(),
                expected.param_info.is_some(),
                "run index: {index}"
            );
            if let Some((a, e)) = actual.param_info.as_ref().zip(expected.param_info.as_ref()) {
                assert_eq!(a.headers, e.headers, "run index: {index}");
                assert_eq!(a.display_str, e.display_str, "run index: {index}");
            }
        }
    }

    #[derive(Default)]
    struct LogActions {
        actions: Vec<TestRunInfo>,
    }

    impl Logger for LogActions {
        fn log(&mut self, test: &'static TestCase, update: TestStatusUpdate, _elapsed: Duration) {
            if let TestStatusUpdate::Started { mut info } = update {
                info.path.push(test.name);
                self.actions.push(info);
            }
        }
    }

    #[test]
    fn simple_tree() {
        assert_tree(
            vec![
                TestCaseBuilder::new("foo"),
                TestCaseBuilder::new("yyy").depends_on(|| FqFnName::new("", "foo")),
                TestCaseBuilder::new("yyy1").depends_on(|| FqFnName::new("", "yyy")),
                TestCaseBuilder::new("yyy2").depends_on(|| FqFnName::new("", "yyy")),
            ],
            vec![
                action("foo", 0),
                action("foo.yyy", 0),
                action("foo.yyy.yyy1", 0),
                action("foo", 1),
                action("foo.yyy", 1),
                action("foo.yyy.yyy2", 0),
            ],
        );
    }

    #[test]
    fn tree_with_cloneable() {
        assert_tree(
            vec![
                TestCaseBuilder::new("boo"),
                TestCaseBuilder::new("xxx")
                    .depends_on(|| FqFnName::new("", "boo"))
                    .set_cloneable(),
                TestCaseBuilder::new("xxx1").depends_on(|| FqFnName::new("", "xxx")),
                TestCaseBuilder::new("xxx2")
                    .depends_on(|| FqFnName::new("", "xxx"))
                    .set_cloneable(),
                TestCaseBuilder::new("xxx1_1").depends_on(|| FqFnName::new("", "xxx1")),
                TestCaseBuilder::new("xxx1_2").depends_on(|| FqFnName::new("", "xxx1")),
                TestCaseBuilder::new("xxx2_1").depends_on(|| FqFnName::new("", "xxx2")),
                TestCaseBuilder::new("xxx2_2").depends_on(|| FqFnName::new("", "xxx2")),
            ],
            vec![
                action("boo", 0),
                action("boo.xxx", 0),
                action("boo.xxx.xxx1", 0),
                action("boo.xxx.xxx1.xxx1_1", 0),
                action("boo.xxx.xxx1", 1),
                action("boo.xxx.xxx1.xxx1_2", 0),
                action("boo.xxx.xxx2", 0),
                action("boo.xxx.xxx2.xxx2_1", 0),
                action("boo.xxx.xxx2.xxx2_2", 0),
            ],
        );
    }

    #[derive(Clone)]
    struct StringArg(&'static str);

    impl ParamDisplay for StringArg {
        const NAMES: &'static [&'static str] = &["name"];

        fn values(&self) -> Vec<String> {
            vec![self.0.to_string()]
        }
    }

    #[test]
    fn tree_with_arguments1() {
        assert_tree(
            vec![
                TestCaseBuilder::new("foo")
                    .with_params(|| TestParams::new([StringArg("A"), StringArg("B")])),
                TestCaseBuilder::new("bar").depends_on(|| FqFnName::new("", "foo")),
            ],
            vec![
                action_with_param("foo", 0, "A"),
                action("foo.bar", 0),
                action_with_param("foo", 0, "B"),
                action("foo.bar", 0),
            ],
        );
    }

    #[test]
    fn tree_with_arguments2() {
        assert_tree(
            vec![
                TestCaseBuilder::new("foo"),
                TestCaseBuilder::new("bar")
                    .depends_on(|| FqFnName::new("", "foo"))
                    .with_params(|| TestParams::new([StringArg("A"), StringArg("B")])),
            ],
            vec![
                action("foo", 0),
                action_with_param("foo.bar", 0, "A"),
                action("foo", 1),
                action_with_param("foo.bar", 0, "B"),
            ],
        );
    }

    #[test]
    fn tree_with_cloneable_and_arguments1() {
        assert_tree(
            vec![
                TestCaseBuilder::new("foo"),
                TestCaseBuilder::new("bar")
                    .depends_on(|| FqFnName::new("", "foo"))
                    .set_cloneable()
                    .with_params(|| TestParams::new([StringArg("A"), StringArg("B")])),
                TestCaseBuilder::new("bar1").depends_on(|| FqFnName::new("", "bar")),
                TestCaseBuilder::new("bar2").depends_on(|| FqFnName::new("", "bar")),
            ],
            vec![
                action("foo", 0),
                action_with_param("foo.bar", 0, "A"),
                action("foo.bar.bar1", 0),
                action("foo.bar.bar2", 0),
                action("foo", 1),
                action_with_param("foo.bar", 0, "B"),
                action("foo.bar.bar1", 0),
                action("foo.bar.bar2", 0),
            ],
        );
    }
}

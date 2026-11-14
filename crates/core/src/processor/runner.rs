use std::iter::once;
use std::time::Instant;

use super::run_state::{RunState, report_failure};
use crate::processor::filter::Filter;
use crate::processor::logger::{Logger, ParamInfo, TestRunInfo};
use crate::test_case::{FqFnName, TestParams, Value};
use crate::tests_tree::{AugmentedTestsTree, TestsTree};

pub struct TestsRunner {}

impl TestsRunner {
    /// Runs the tree one leaf at a time. A leaf starts from what the tests above it built, so
    /// those are executed again for every leaf, except where a test kept what it built.
    pub async fn run_tests(tree: &TestsTree, filter: &dyn Filter, logger: &mut dyn Logger) {
        let tree = AugmentedTestsTree::new(tree, |node| {
            node.params
                .as_ref()
                .map(|p| (p.params)())
                .unwrap_or_else(TestParams::new_empty)
        });
        let mut execution = Execution {
            filter,
            logger,
            started_at: Instant::now(),
        };

        // The tests from the root down to the leaf that runs next.
        let mut chain = vec![TestState::new(vec![], &tree)];
        while !chain.is_empty() {
            while let Some(child) = chain.last().and_then(|test| test.next_child()) {
                chain.push(child);
            }
            execution.run_leaf(&mut chain).await;
            // On to the next leaf: leave every test that has nothing more to run below it.
            while chain.last_mut().is_some_and(|test| !test.advance()) {
                chain.pop();
            }
        }
    }
}

struct Execution<'a> {
    filter: &'a dyn Filter,
    logger: &'a mut dyn Logger,
    started_at: Instant,
}

impl Execution<'_> {
    /// Runs the last test of `chain`, after building what it starts from: the tests above it
    /// are executed again, down from the nearest one that kept what it built.
    async fn run_leaf(&mut self, chain: &mut [TestState<'_>]) {
        let (leaf, above) = chain
            .split_last_mut()
            .expect("a chain has at least its root");
        let resume_at = above.iter().rposition(|test| test.kept.is_some());
        let mut run_state = RunState::init();
        for test in &mut above[resume_at.unwrap_or(0)..] {
            let built = match test.kept.take() {
                Some(kept) => kept,
                None => self.execute(test, run_state).await,
            };
            run_state = self.hand_on(test, built).await;
        }
        self.execute(leaf, run_state).await;
    }

    async fn execute(&mut self, test: &mut TestState<'_>, run_state: RunState) -> RunState {
        let (info, param_value) = test.prepare_test_run();
        test.run_count += 1;
        run_state
            .run_test(
                self.filter,
                self.logger,
                test.tree.node.test,
                self.started_at,
                info,
                param_value,
            )
            .await
    }

    /// Hands what `test` built to the test below it. Where a copy can be made, the copy is
    /// handed on and the original is kept, so that `test` is not executed again for the next
    /// test below it.
    async fn hand_on(&mut self, test: &mut TestState<'_>, built: RunState) -> RunState {
        let case = test.tree.node.test;
        match built.clone_state(case.clone.as_ref()).await {
            Ok(Some(copy)) => {
                test.kept = Some(built);
                copy
            }
            Ok(None) => built,
            Err(reason) => {
                // The test passed, but what it built cannot be handed on. That is a failure
                // of this test, and it is not tried again for the tests that follow.
                let (info, _) = test.prepare_test_run();
                report_failure(self.logger, case, self.started_at, info, &reason);
                test.kept = Some(RunState::Skip(reason.clone()));
                RunState::Skip(reason)
            }
        }
    }
}

struct TestState<'a> {
    tree: &'a AugmentedTestsTree<TestParams>,
    path: Vec<FqFnName<'static>>,
    args_idx: usize,
    childs_idx: usize,
    run_count: usize,
    /// What the test built with its current parameter, kept for the tests below it that are
    /// still to run.
    kept: Option<RunState>,
}

impl<'a> TestState<'a> {
    fn new(path: Vec<FqFnName<'static>>, tree: &'a AugmentedTestsTree<TestParams>) -> Self {
        Self {
            tree,
            path,
            args_idx: 0,
            childs_idx: 0,
            run_count: 0,
            kept: None,
        }
    }

    fn prepare_test_run(&self) -> (TestRunInfo, Option<Value>) {
        let (param_info, param_value) = if self.args_idx < self.tree.node.extra.len() {
            let arg = self.tree.node.extra.get(self.args_idx);
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
                run_count: self.run_count,
                param_info,
            },
            param_value,
        )
    }

    /// The child that runs next below this test, if this test is not a leaf.
    fn next_child(&self) -> Option<TestState<'a>> {
        let child = self.tree.childs.get(self.childs_idx)?;
        let path = self
            .path
            .iter()
            .copied()
            .chain(once(self.tree.node.test.name))
            .collect();
        Some(TestState::new(path, child))
    }

    /// Moves to the next child, or, after the last one, back to the first with the next
    /// parameter. Returns `false` when nothing is left to run through this test.
    fn advance(&mut self) -> bool {
        self.childs_idx += 1;
        if self.childs_idx < self.tree.childs.len() {
            return true;
        }
        self.args_idx += 1;
        if self.args_idx < self.tree.node.extra.len() {
            self.childs_idx = 0;
            self.run_count = 0;
            self.kept = None;
            return true;
        }
        false
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

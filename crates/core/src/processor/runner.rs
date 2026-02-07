use std::time::Instant;

use super::run_state::RunState;
use crate::processor::filter::Filter;
use crate::processor::logger::{Logger, ParamInfo, TestRunInfo};
use crate::test_case::{TestCase, TestParams, Value};
use crate::tests_tree::TestsTree;

pub struct TestsRunner {}

impl TestsRunner {
    pub async fn run_tests(tree: &TestsTree, filter: &dyn Filter, logger: &mut dyn Logger) {
        let tree: TestsTreeWithState = TestsTreeWithState::new(tree.clone());
        let mut state = Vec::new();
        let mut depth = 0;
        // tree.process = Some(TestProcess::Process { run_state: RunState::init() });
        // state.push(tree);
        state.push(TestState::new(&tree, RunState::init()));

        #[derive(Clone, Copy)]
        enum ProgressAction {
            ProcessChain,
            FindBranch,
        }

        let started_at = Instant::now();

        let mut action = ProgressAction::ProcessChain;

        while !state.is_empty() {
            let exec_state_len = state.len();
            let (processed, remaining) = state.split_at_mut(depth + 1);
            let curr = processed.last_mut().unwrap();
            if let (ProgressAction::ProcessChain, true) = (action, curr.state.process.is_some()) {
                let process = curr.state.process.take().unwrap();
                let new_run = match process {
                    TestProcess::Process { run_state } => {
                        let (info, param_value) = curr.prepare_test_run(depth);

                        curr.state.run_count += 1;
                        run_state
                            .run_test(
                                filter,
                                logger,
                                curr.test.node,
                                started_at,
                                info,
                                &curr.test.node.test_fn,
                                &curr.test.node.env,
                                param_value,
                            )
                            .await
                    }
                    TestProcess::AlreadyProcessed { outcome } => outcome,
                };

                if let Some(clone_fns) = curr.test.node.clone.as_ref() {
                    if should_clone(curr, remaining) {
                        curr.state.process = Some(TestProcess::AlreadyProcessed {
                            outcome: new_run.clone_state(clone_fns),
                        });
                    }
                }

                if exec_state_len > depth + 1 {
                    depth += 1;
                    state[depth].state.process = Some(TestProcess::Process { run_state: new_run });
                } else if curr.state.childs_idx < curr.test.childs.len() {
                    let next = &curr.test.childs[curr.state.childs_idx];
                    depth += 1;
                    state.push(TestState::new(next, new_run));
                } else {
                    action = ProgressAction::FindBranch;
                }
            } else {
                if let ProgressAction::FindBranch = action {
                    curr.advance();
                    if !curr.is_processed() {
                        action = ProgressAction::ProcessChain;
                        depth += 1;
                    } else {
                        state.pop();
                    }
                }

                if depth > 0 {
                    depth -= 1;
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

struct TestsTreeWithState {
    node: &'static TestCase,
    params: TestParams,
    childs: Vec<TestsTreeWithState>,
}

impl TestsTreeWithState {
    fn new(tree: TestsTree) -> Self {
        TestsTreeWithState {
            node: tree.node,
            params: tree
                .node
                .params
                .as_ref()
                .map(|p| (p.params)())
                .unwrap_or_else(|| TestParams::new_empty()),
            childs: tree.childs.into_iter().map(Self::new).collect(),
        }
    }
}

struct State {
    process: Option<TestProcess>,
    args_idx: usize,
    childs_idx: usize,
    run_count: usize,
}

struct TestState<'a> {
    test: &'a TestsTreeWithState,
    state: State,
}

impl<'a> TestState<'a> {
    fn new(test: &'a TestsTreeWithState, run_state: RunState) -> Self {
        Self {
            test,
            state: State {
                process: Some(TestProcess::Process { run_state }),
                args_idx: 0,
                childs_idx: 0,
                run_count: 0,
            },
        }
    }

    fn prepare_test_run(&self, depth: usize) -> (TestRunInfo, Option<Value>) {
        let (param_info, param_value) = if self.state.args_idx < self.test.params.len() {
            let arg = self.test.params.get(self.state.args_idx);
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
                depth,
                run_count: self.state.run_count,
                param_info,
            },
            param_value,
        )
    }

    fn is_processed(&self) -> bool {
        self.test.childs.len() == self.state.childs_idx
            && self.test.params.len() == self.state.args_idx
    }

    fn advance(&mut self) {
        if self.state.childs_idx < self.test.childs.len() {
            self.state.childs_idx += 1;
            if self.state.childs_idx == self.test.childs.len()
                && self.state.args_idx < self.test.params.len()
            {
                self.state.args_idx += 1;
                self.state.run_count = 0;
                self.state.process = None;
                if self.state.args_idx < self.test.params.len() {
                    self.state.childs_idx = 0;
                }
            }
        } else if self.state.args_idx < self.test.params.len() {
            self.state.args_idx += 1;
            self.state.run_count = 0;
            self.state.process = None;
            self.state.childs_idx = 0;
        }
    }
}

fn should_clone(curr: &TestState, in_progress: &[TestState]) -> bool {
    // for current test we only care if it has more childs
    if curr.state.childs_idx + 1 < curr.test.childs.len() {
        return true;
    }

    // for in progress tests additionally check for arguments
    // NOTE: cloneable tests cannot be in this list
    for next in in_progress {
        if next.state.childs_idx + 1 < next.test.childs.len() {
            return true;
        }
        if next.state.args_idx + 1 < next.test.params.len() {
            return true;
        }
    }

    let remaining = in_progress
        .last()
        .map(|t| t.test.childs.split_at(t.state.childs_idx).1)
        .unwrap_or_else(|| curr.test.childs.split_at(curr.state.childs_idx).1);
    should_clone_recursive(remaining)
}

fn should_clone_recursive(childs: &[TestsTreeWithState]) -> bool {
    for c in childs {
        if c.node.clone.is_none() {
            if c.childs.len() > 1 {
                return true;
            }
        } else if c.params.len() > 1 {
            return true;
        }
        if should_clone_recursive(&c.childs) {
            return true;
        }
    }
    false
}

#[cfg(test)]
pub mod tests {

    use std::time::Duration;

    use futures::executor::block_on;

    use super::*;
    use crate::processor::filter::NoFilter;
    use crate::processor::logger::TestStatusUpdate;
    use crate::test_case::{CloneFns, FqFnName, TestCase, TestFn, Value};
    use crate::tests_tree::create_test_trees;

    struct TestCaseBuilder {
        fn_name: FqFnName<'static>,
        is_cloneable: bool,
    }

    impl TestCaseBuilder {
        const fn new(fn_name: &'static str) -> Self {
            Self {
                fn_name: FqFnName::new("", fn_name),
                is_cloneable: false,
            }
        }

        const fn set_cloneable(mut self) -> Self {
            self.is_cloneable = true;
            self
        }

        const fn depends_on(self, _path_with_name: &'static str) -> Self {
            // TODO implement, but probably not possible...
            // we should probably write integration tests instead
            self
        }
        const fn build(self) -> TestCase {
            TestCase {
                name: self.fn_name,
                tags: &[],
                filename: "",
                line_nr: 0,
                test_fn: TestFn::SyncFn(|_, v, _e, _p| v),
                parent: None,
                env: None,
                params: None,
                clone: if self.is_cloneable {
                    Some(CloneFns {
                        state: |_| Value::new(()),
                        env: |_| Value::new(()),
                    })
                } else {
                    None
                },
            }
        }
    }

    #[derive(Default)]
    struct LogActions {
        actions: Vec<(&'static str, usize, usize)>,
    }

    impl Logger for LogActions {
        fn log(&mut self, test: &'static TestCase, update: TestStatusUpdate, _elapsed: Duration) {
            if let TestStatusUpdate::Started { info } = update {
                self.actions
                    .push((test.name.name, info.depth, info.run_count));
            }
        }
    }

    #[test]
    fn test_new_visitor() {
        // boo  -> xxx(c)   -> xxx1     -> xxx1_1
        //                              -> xxx1_2
        //                  -> xxx2(c)  -> xxx2_1
        //                              -> xxx2_2
        // foo  -> yyy  -> yyy1
        //              -> yyy2
        static CASES: &[TestCase] = &[
            TestCaseBuilder::new("boo").build(),
            TestCaseBuilder::new("foo").build(),
            TestCaseBuilder::new("xxx")
                .depends_on("boo")
                .set_cloneable()
                .build(),
            TestCaseBuilder::new("yyy").depends_on("foo").build(),
            TestCaseBuilder::new("xxx1").depends_on("xxx").build(),
            TestCaseBuilder::new("xxx2")
                .depends_on("xxx")
                .set_cloneable()
                .build(),
            TestCaseBuilder::new("yyy1").depends_on("yyy").build(),
            TestCaseBuilder::new("yyy2").depends_on("yyy").build(),
            TestCaseBuilder::new("xxx1_1").depends_on("xxx1").build(),
            TestCaseBuilder::new("xxx1_2").depends_on("xxx1").build(),
            TestCaseBuilder::new("xxx2_1").depends_on("xxx2").build(),
            TestCaseBuilder::new("xxx2_2").depends_on("xxx2").build(),
        ];
        let mut trees = create_test_trees(CASES);
        for tree in trees.values() {
            tree.verify(false).unwrap();
        }
        let mut logger = LogActions::default();
        block_on(TestsRunner::run_tests(
            &trees.pop_first().unwrap().1,
            &NoFilter,
            &mut logger,
        ));

        block_on(TestsRunner::run_tests(
            &trees.pop_first().unwrap().1,
            &NoFilter,
            &mut logger,
        ));
    }

    #[test]
    #[ignore = "disable while migrating from DependsOn to proper ParentFn"]
    fn run_tests_no_params() {
        // boo  -> xxx(c)   -> xxx1     -> xxx1_1
        //                              -> xxx1_2
        //                  -> xxx2(c)  -> xxx2_1
        //                              -> xxx2_2
        // foo  -> yyy  -> yyy1
        //              -> yyy2
        static CASES: &[TestCase] = &[
            TestCaseBuilder::new("boo").build(),
            TestCaseBuilder::new("foo").build(),
            TestCaseBuilder::new("xxx")
                .depends_on("boo")
                .set_cloneable()
                .build(),
            TestCaseBuilder::new("yyy").depends_on("foo").build(),
            TestCaseBuilder::new("xxx1").depends_on("xxx").build(),
            TestCaseBuilder::new("xxx2")
                .depends_on("xxx")
                .set_cloneable()
                .build(),
            TestCaseBuilder::new("yyy1").depends_on("yyy").build(),
            TestCaseBuilder::new("yyy2").depends_on("yyy").build(),
            TestCaseBuilder::new("xxx1_1").depends_on("xxx1").build(),
            TestCaseBuilder::new("xxx1_2").depends_on("xxx1").build(),
            TestCaseBuilder::new("xxx2_1").depends_on("xxx2").build(),
            TestCaseBuilder::new("xxx2_2").depends_on("xxx2").build(),
        ];
        let mut trees = create_test_trees(CASES);
        for tree in trees.values() {
            tree.verify(false).unwrap();
        }
        let mut logger = LogActions::default();
        block_on(TestsRunner::run_tests(
            &trees.pop_first().unwrap().1,
            &NoFilter,
            &mut logger,
        ));

        assert_eq!(
            logger.actions,
            vec![
                ("boo", 0, 0),
                ("xxx", 1, 0),
                ("xxx1", 2, 0),
                ("xxx1_1", 3, 0),
                ("xxx1", 2, 1),
                ("xxx1_2", 3, 0),
                ("xxx2", 2, 0),
                ("xxx2_1", 3, 0),
                ("xxx2_2", 3, 0),
            ]
        );
        let mut logger = LogActions::default();
        block_on(TestsRunner::run_tests(
            &trees.pop_first().unwrap().1,
            &NoFilter,
            &mut logger,
        ));
        assert_eq!(
            logger.actions,
            vec![
                ("foo", 0, 0),
                ("yyy", 1, 0),
                ("yyy1", 2, 0),
                ("foo", 0, 1),
                ("yyy", 1, 1),
                ("yyy2", 2, 0)
            ]
        );
    }

    // #[test]
    // fn run_tests_with_params() {
    //     // boo  -> xxx(c)   -> xxx1     -> xxx1_1
    //     //                              -> xxx1_2
    //     //                  -> xxx2(c)  -> xxx2_1
    //     //                              -> xxx2_2
    //     // foo  -> yyy  -> yyy1
    //     //              -> yyy2
    //     static CASES: &[TestCase] = &[
    //         TestCaseBuilder::new("boo").build(),
    //         TestCaseBuilder::new("foo").build(),
    //         TestCaseBuilder::new("xxx")
    //             .depends_on("boo")
    //             .set_cloneable()
    //             .build(),
    //         TestCaseBuilder::new("yyy").depends_on("foo").build(),
    //         TestCaseBuilder::new("xxx1").depends_on("xxx").build(),
    //         TestCaseBuilder::new("xxx2")
    //             .depends_on("xxx")
    //             .set_cloneable()
    //             .build(),
    //         TestCaseBuilder::new("yyy1").depends_on("yyy").build(),
    //         TestCaseBuilder::new("yyy2").depends_on("yyy").build(),
    //         TestCaseBuilder::new("xxx1_1").depends_on("xxx1").build(),
    //         TestCaseBuilder::new("xxx1_2").depends_on("xxx1").build(),
    //         TestCaseBuilder::new("xxx2_1").depends_on("xxx2").build(),
    //         TestCaseBuilder::new("xxx2_2").depends_on("xxx2").build(),
    //     ];
    //     let mut trees = create_test_trees(CASES).unwrap();
    //     for tree in trees.values() {
    //         tree.verify(false).unwrap();
    //     }
    //     let mut logger = LogActions::default();
    //     block_on(TestsVisitor::visit(
    //         &trees.pop_first().unwrap().1,
    //         &NoFilter,
    //         &mut logger,
    //     ));

    //     assert_eq!(
    //         logger.actions,
    //         vec![
    //             ("boo", 0, 0),
    //             ("xxx", 1, 0),
    //             ("xxx1", 2, 0),
    //             ("xxx1_1", 3, 0),
    //             ("xxx1", 2, 1),
    //             ("xxx1_2", 3, 0),
    //             ("xxx2", 2, 0),
    //             ("xxx2_1", 3, 0),
    //             ("xxx2_2", 3, 0),
    //         ]
    //     );
    //     let mut logger = LogActions::default();
    //     block_on(TestsVisitor::visit(
    //         &trees.pop_first().unwrap().1,
    //         &NoFilter,
    //         &mut logger,
    //     ));
    //     assert_eq!(
    //         logger.actions,
    //         vec![
    //             ("foo", 0, 0),
    //             ("yyy", 1, 0),
    //             ("yyy1", 2, 0),
    //             ("foo", 0, 1),
    //             ("yyy", 1, 1),
    //             ("yyy2", 2, 0)
    //         ]
    //     );
    // }

    // #[test]
    // fn run_tests_with_params_and_cloneable() {
    //     static CASES: &[TestCase] = &[
    //         TestCaseBuilder::new("root").build(),
    //         TestCaseBuilder::new("boo")
    //             .depends_on("root")
    //             .set_cloneable()
    //             .params("p1")
    //             .build(),
    //         TestCaseBuilder::new("foo")
    //             .depends_on("boo")
    //             .params("p2")
    //             .build(),
    //     ];
    //     static PARAMS: &[TestParams] = &[
    //         TestParamsBuilder::<2>::new("p1").build(),
    //         TestParamsBuilder::<2>::new("p2").build(),
    //     ];
    //     let mut trees = create_test_trees(CASES, PARAMS).unwrap();
    //     for tree in trees.values() {
    //         tree.verify(false).unwrap();
    //     }
    //     let mut processor = LogActions::default();
    //     block_on(TestsVisitor::visit(
    //         &trees.pop_first().unwrap().1,
    //         &NoFilter,
    //         &mut processor,
    //     ));

    //     assert_eq!(
    //         processor.actions,
    //         vec![
    //             ("root", 1, true),
    //             ("boo", 2, true),
    //             ("foo", 3, true),
    //             ("foo", 3, true),
    //             ("root", 1, false),
    //             ("boo", 2, true),
    //             ("foo", 3, true),
    //             ("foo", 3, true)
    //         ]
    //     );
    // }
}

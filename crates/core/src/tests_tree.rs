use std::collections::HashMap;

use serde::Serialize;
use thiserror::Error;

use crate::test_case::{FqFnName, TestCase, name_from_type};

#[derive(Error, Debug)]
pub enum BuildTreeError {
    #[error("Initial environment `Base` must be of type `()`")]
    EnvironmentBaseMismatch {
        current_test: FqFnName<'static>,
        env_name: FqFnName<'static>,
        expected_base: FqFnName<'static>,
        actual_base: FqFnName<'static>,
    },
    #[error("Test `{test}` is async, but runtime is not async")]
    AsyncRuntimeRequired { test: FqFnName<'static> },
    #[error(
        "Test `{test}` is async={test_is_async}, parent `{parent}` is async={parent_is_async}."
    )]
    AsyncnessMismatch {
        parent: FqFnName<'static>,
        parent_is_async: bool,
        test: FqFnName<'static>,
        test_is_async: bool,
    },
}

#[derive(Debug, Clone, Serialize)]
pub struct TestsTree {
    pub node: &'static TestCase,
    pub childs: Vec<TestsTree>,
}

impl TestsTree {
    pub fn visit(&self, f: &mut dyn FnMut(&'static TestCase, usize)) {
        f(&self.node, 0);
        for c in &self.childs {
            c.visit_with_depth(f, 1);
        }
    }

    fn visit_with_depth(&self, f: &mut dyn FnMut(&'static TestCase, usize), depth: usize) {
        f(&self.node, depth);
        for c in &self.childs {
            c.visit_with_depth(f, depth + 1);
        }
    }

    pub fn verify(&self, is_async_runtime: bool) -> Result<(), BuildTreeError> {
        if self.node.test_fn.is_async() && !is_async_runtime {
            return Err(BuildTreeError::AsyncRuntimeRequired {
                test: self.node.name,
            });
        }
        self.verify_asyncness(self.node.test_fn.is_async())?;
        self.verify_env(name_from_type::<()>())?;
        Ok(())
    }

    fn verify_asyncness(&self, root_async: bool) -> Result<(), BuildTreeError> {
        for child in &self.childs {
            if child.node.test_fn.is_async() != root_async {
                return Err(BuildTreeError::AsyncnessMismatch {
                    parent: self.node.name,
                    parent_is_async: root_async,
                    test: child.node.name,
                    test_is_async: child.node.test_fn.is_async(),
                });
            }
            child.verify_asyncness(root_async)?;
        }
        Ok(())
    }

    fn verify_env(&self, base: FqFnName<'static>) -> Result<(), BuildTreeError> {
        let env_name = if let Some(env) = &self.node.env {
            let env_name = (env.self_type)();
            let expected_base = (env.base_type)();
            if expected_base != base && env_name != base {
                return Err(BuildTreeError::EnvironmentBaseMismatch {
                    current_test: self.node.name,
                    env_name,
                    expected_base,
                    actual_base: base,
                });
            }
            env_name
        } else {
            name_from_type::<()>()
        };

        for child in &self.childs {
            child.verify_env(env_name)?;
        }
        Ok(())
    }

    fn assign_childs(&mut self, childs: &mut HashMap<FqFnName, Vec<&'static TestCase>>) {
        if let Some((_fq, list)) = childs.remove_entry(&self.node.name) {
            for child in list {
                let mut dag = TestsTree {
                    node: child,
                    childs: vec![],
                };
                dag.assign_childs(childs);
                self.childs.push(dag);
            }
        }
    }
}

pub fn create_test_trees(test_cases: &'static [TestCase]) -> Vec<TestsTree> {
    let mut sorted: Vec<_> = test_cases.iter().collect();
    sorted.sort_unstable_by(|a, b| a.parent.is_some().cmp(&b.parent.is_some()));
    let part_index = sorted.partition_point(|a| a.parent.is_none());
    let (roots_list, childs_list) = sorted.split_at_mut(part_index);
    // roots are sorted by name to ensure deterministic order of root tests,
    roots_list.sort_unstable_by_key(|a| a.name);
    // childs are sorted by order they are defined, to for easier development experience, but also to control what outcome of test tree
    // e.g. cover most important cases first
    childs_list.sort_unstable_by(|a, b| (a.filename, a.line_nr).cmp(&(b.filename, b.line_nr)));
    let mut childs: HashMap<FqFnName<'static>, Vec<&'static TestCase>> = HashMap::new();
    for child in childs_list {
        childs
            .entry((child.parent.as_ref().unwrap().get_name)())
            .or_default()
            .push(child);
    }
    let mut roots = roots_list
        .into_iter()
        .map(|node| TestsTree {
            node,
            childs: Default::default(),
        })
        .collect::<Vec<_>>();

    for tree in &mut roots {
        tree.assign_childs(&mut childs);
    }
    roots
}

/// Filters test trees by removing tests that don't match the filter function.
/// Parent tests are automatically kept if any of their children pass the filter,
/// ensuring the full test hierarchy remains intact.
/// `filter_out_fn` should return true for tests to be filtered out.
pub fn filter_test_trees(
    trees: Vec<TestsTree>,
    filter_out_fn: impl Fn(&'static TestCase) -> bool,
) -> Vec<TestsTree> {
    let mut tests = HashMap::new();

    for tree in &trees {
        build_filter_info(tree, None, &filter_out_fn, &mut tests);
    }

    // unfilter parents of non-filtered tests
    for tree in &trees {
        unfilter_parents(tree, &mut tests);
    }

    trees
        .into_iter()
        .filter_map(|tree| remove_filtered_tests(&tree, &tests))
        .collect()
}

#[derive(Debug)]
struct TestFilterInfo {
    filtered: bool,
    parent: Option<FqFnName<'static>>,
}

fn remove_filtered_tests(
    tree: &TestsTree,
    tests: &HashMap<FqFnName<'static>, TestFilterInfo>,
) -> Option<TestsTree> {
    let info = tests.get(&tree.node.name).unwrap();
    if info.filtered {
        return None;
    }

    let mut new_tree = TestsTree {
        node: tree.node,
        childs: vec![],
    };
    for child in &tree.childs {
        if let Some(new_child) = remove_filtered_tests(child, tests) {
            new_tree.childs.push(new_child);
        }
    }
    Some(new_tree)
}

fn build_filter_info(
    tree: &TestsTree,
    parent: Option<FqFnName<'static>>,
    filter_out_fn: &dyn Fn(&'static TestCase) -> bool,
    tests: &mut HashMap<FqFnName<'static>, TestFilterInfo>,
) {
    tests.insert(
        tree.node.name,
        TestFilterInfo {
            parent,
            filtered: filter_out_fn(&tree.node),
        },
    );
    for child in &tree.childs {
        build_filter_info(child, Some(tree.node.name), filter_out_fn, tests);
    }
}

fn unfilter_parents(tree: &TestsTree, tests: &mut HashMap<FqFnName<'static>, TestFilterInfo>) {
    let info = tests.get(&tree.node.name).unwrap();
    if !info.filtered {
        let mut parent = info.parent;
        while let Some(parent_name) = parent {
            let parent_info = tests.get_mut(&parent_name).unwrap();
            if !parent_info.filtered {
                break;
            }
            parent_info.filtered = false;
            parent = parent_info.parent;
        }
    }

    for child in &tree.childs {
        unfilter_parents(child, tests);
    }
}

use std::collections::BTreeMap;

use serde::Serialize;
use thiserror::Error;

use crate::test_case::{FqFnName, TestCase};

#[derive(Error, Debug)]
pub enum BuildDagError {
    #[error("Initial environment creation cannot have any arguments ({env_name})")]
    EnvironmentInitiationWithArgument { env_name: FqFnName<'static> },
    #[error(
        "Test `{current_test}` failed to create a new environment because it expected the previous environment to be of type `{current_env_init_type}`, but found `{parent_env_type}` instead."
    )]
    EnvironmentTransformMismatch {
        parent_env_type: &'static str,
        current_test: FqFnName<'static>,
        current_env_init_type: &'static str,
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
    pub fn visit(&self, f: &mut dyn FnMut(&'static TestCase)) {
        f(&self.node);
        for c in &self.childs {
            c.visit(f);
        }
    }

    pub fn verify(&self, is_async_runtime: bool) -> Result<(), BuildDagError> {
        if self.node.test_fn.is_async() && !is_async_runtime {
            return Err(BuildDagError::AsyncRuntimeRequired {
                test: self.node.name,
            });
        }
        self.verify_asyncness(self.node.test_fn.is_async())?;

        if let Some(env) = &self.node.env {
            if ((env.arg_type)()) != "()" {
                return Err(BuildDagError::EnvironmentInitiationWithArgument {
                    env_name: (env.get_name)(),
                });
            }
        }
        self.verify_env()?;
        Ok(())
    }

    fn verify_asyncness(&self, root_async: bool) -> Result<(), BuildDagError> {
        for child in &self.childs {
            if child.node.test_fn.is_async() != root_async {
                return Err(BuildDagError::AsyncnessMismatch {
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

    fn verify_env(&self) -> Result<(), BuildDagError> {
        for child in &self.childs {
            if let Some(env) = &child.node.env {
                if let Some(this_env) = &self.node.env {
                    if (this_env.get_name)() != (env.get_name)()
                        && (env.arg_type)() != (this_env.return_type)()
                    {
                        return Err(BuildDagError::EnvironmentTransformMismatch {
                            parent_env_type: (this_env.return_type)(),
                            current_test: child.node.name,
                            current_env_init_type: (env.arg_type)(),
                        });
                    }
                } else {
                    if ((env.arg_type)()) != "()" {
                        return Err(BuildDagError::EnvironmentTransformMismatch {
                            parent_env_type: "()",
                            current_test: child.node.name,
                            current_env_init_type: (env.arg_type)(),
                        });
                    }
                }
            }
            child.verify_env()?;
        }
        Ok(())
    }

    fn assign_childs(&mut self, childs: &mut BTreeMap<FqFnName, Vec<&'static TestCase>>) {
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

pub fn create_test_trees(
    test_cases: &'static [TestCase],
) -> Result<BTreeMap<FqFnName<'static>, TestsTree>, BuildDagError> {
    let GroupedTests {
        mut roots,
        mut childs,
    } = get_roots_and_childs(test_cases)?;

    for tree in &mut roots.values_mut() {
        tree.assign_childs(&mut childs);
    }

    Ok(roots)
}

#[derive(Default)]
struct GroupedTests {
    roots: BTreeMap<FqFnName<'static>, TestsTree>,
    childs: BTreeMap<FqFnName<'static>, Vec<&'static TestCase>>,
}

fn get_roots_and_childs(all: &'static [TestCase]) -> Result<GroupedTests, BuildDagError> {
    let mut res = GroupedTests::default();
    for t in all {
        if let Some(parent) = t.parent.as_ref() {
            res.childs.entry((parent.get_name)()).or_default().push(t);
        } else {
            res.roots.insert(
                t.name,
                TestsTree {
                    node: t,
                    childs: vec![],
                },
            );
        }
    }
    Ok(res)
}

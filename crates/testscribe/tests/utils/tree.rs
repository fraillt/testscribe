use std::collections::BTreeMap;

use testscribe::test_case::{FqFnName, TestCase};
use testscribe::tests_tree::{BuildTreeError, TestsTree, create_test_trees};

pub fn create_and_verify_tt(
    test_cases: &'static [TestCase],
    is_async_runtime: bool,
) -> Result<BTreeMap<FqFnName<'static>, TestsTree>, BuildTreeError> {
    let trees = create_test_trees(test_cases);
    for tree in trees.values() {
        tree.verify(is_async_runtime)?;
    }

    Ok(trees)
}

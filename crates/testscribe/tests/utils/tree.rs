use testscribe::test_case::TestCase;
use testscribe::tests_tree::{BuildTreeError, TestsTree, create_test_trees};

pub fn create_and_verify_tt(
    test_cases: &'static [TestCase],
    is_async_runtime: bool,
) -> Result<Vec<TestsTree>, BuildTreeError> {
    let trees = create_test_trees(test_cases);
    for tree in &trees {
        tree.verify(is_async_runtime)?;
    }

    Ok(trees)
}

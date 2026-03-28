// TODO this should be private, but it exposed for backend
pub mod args;
pub mod filter;
pub mod logger;
pub mod panic_hook;
mod runner;

use std::{io::Write, sync::Mutex};

use futures::executor::block_on;

use testscribe_core::{
    processor::{
        filter::{Filter, NoFilter},
        logger::TestRunInfo,
    },
    test_case::{FqFnName, TestCase},
    tests_tree::{BuildTreeError, TestsTree, create_test_trees, filter_test_trees},
};

use crate::{
    args::Arguments,
    filter::{IGNORE_TAG_NAME, filter_out_test},
    logger::{printer::TestFormatter, summary::ExecutionSummary},
};

pub use crate::runner::run_test_tree;

/// This function signature is used by proc-macros to run `standalone` tests synchronously.
pub fn run_sync(
    test_cases: &'static [TestCase],
    module_path: &'static str,
    test_name: &'static str,
) -> Result<(), String> {
    let mut trees = create_test_trees(test_cases);
    let tree_name = &FqFnName::new(module_path, test_name);
    let tree = trees.swap_remove(
        trees
            .binary_search_by(|tree| tree.node.name.cmp(tree_name))
            .unwrap(),
    );
    tree.verify(false).map_err(|err| err.to_string())?;

    let args = Arguments::from_args();
    let mut stdout = std::io::stdout();
    let mut vecout = Vec::new();
    let output: &mut dyn Write = if args.nocapture {
        &mut stdout
    } else {
        &mut vecout
    };

    let mut printer = TestFormatter::new(output);
    let summary = block_on(run_test_tree(tree, &NoFilter, &mut printer, args.exact));

    printer.print_failures(&summary.failed);
    printer.print_panics(&summary.panics);
    if !args.nocapture {
        print!("{}", String::from_utf8(vecout).unwrap());
    }

    if summary.is_success() {
        Ok(())
    } else {
        return Err(format!("Test {test_name} failed."));
    }
}

/// This function signature is used by proc-macros to run `standalone` tests asynchronously.
pub async fn run_async(
    test_cases: &'static [TestCase],
    module_path: &'static str,
    test_name: &'static str,
) -> Result<(), String> {
    let mut trees = create_test_trees(test_cases);
    let tree_name = &FqFnName::new(module_path, test_name);
    let tree = trees.swap_remove(
        trees
            .binary_search_by(|tree| tree.node.name.cmp(tree_name))
            .unwrap(),
    );
    tree.verify(true).map_err(|err| err.to_string())?;

    let args = Arguments::from_args();
    let mut stdout = std::io::stdout();
    let mut vecout = Vec::new();
    let output: &mut dyn Write = if args.nocapture {
        &mut stdout
    } else {
        &mut vecout
    };

    let mut printer = TestFormatter::new(output);
    let summary = run_test_tree(tree, &NoFilter, &mut printer, args.exact).await;
    printer.print_failures(&summary.failed);
    printer.print_panics(&summary.panics);
    if !args.nocapture {
        print!("{}", String::from_utf8(vecout).unwrap());
    }
    if summary.is_success() {
        Ok(())
    } else {
        return Err(format!("Test {test_name} failed."));
    }
}

/// Runs tests similarly to `libtest`,
/// However since we're running test trees, some behaviour is different:
///
pub fn run_all_sync(
    test_cases: &'static [TestCase],
    args: Arguments,
) -> Result<ExecutionSummary, BuildTreeError> {
    let trees = filter_test_trees(create_test_trees(test_cases), |test| {
        filter_out_test(test, &args)
    });
    trees.iter().try_for_each(|tree| tree.verify(false))?;

    if args.list {
        for tree in &trees {
            print_list(&tree, args.ignored, 0);
        }
        return Ok(ExecutionSummary::default());
    }

    let num_threads = platform_defaults_to_one_thread()
        .then_some(1)
        .or(args.test_threads)
        .or_else(|| std::thread::available_parallelism().ok().map(Into::into))
        .unwrap_or(1);

    let filter = FilterIgnored { args: args.clone() };

    if num_threads == 1 {
        let mut summary = ExecutionSummary::default();
        let mut stdout = std::io::stdout();
        let mut vecout = Vec::new();
        let output: &mut dyn Write = if args.nocapture {
            &mut stdout
        } else {
            &mut vecout
        };

        let mut printer = TestFormatter::new(output);
        for tree in trees {
            let tree_summary = block_on(run_test_tree(tree, &filter, &mut printer, true));
            summary.extend(&tree_summary);

            if args.fail_fast && !summary.is_success() {
                break;
            }
        }
        printer.print_failures(&summary.failed);
        printer.print_panics(&summary.panics);
        Ok(summary)
    } else {
        // libtest_mimic::run(args, tests)

        let (sender, receiver) = std::sync::mpsc::channel();
        let num_roots = trees.len();
        let num_threads = num_threads.min(num_roots);
        let iter = Mutex::new(trees.into_iter());
        std::thread::scope(|scope| {
            // Start worker threads
            for _ in 0..num_threads {
                scope.spawn(|| {
                    loop {
                        // Get next test to process from the iterator.
                        let Some(tree) = iter.lock().unwrap().next() else {
                            break;
                        };

                        let mut vecout = Vec::new();
                        let mut printer = TestFormatter::new(&mut vecout);
                        let summary = block_on(run_test_tree(tree, &filter, &mut printer, true));

                        sender.send((summary, vecout)).unwrap();
                    }
                });
            }
        });

        let mut summary = ExecutionSummary::default();
        let mut iter = receiver.try_iter().take(num_roots);
        while let Some((test_summary, vecout)) = iter.next() {
            summary.extend(&test_summary);
            print!("{}", String::from_utf8(vecout).unwrap());
        }
        let mut stdout = std::io::stdout();
        let mut printer = TestFormatter::new(&mut stdout);
        printer.print_failures(&summary.failed);
        printer.print_panics(&summary.panics);
        Ok(summary)
    }
}

/// Returns whether the current host platform should use a single thread by
/// default rather than a thread pool by default. Some platforms, such as
/// WebAssembly, don't have native support for threading at this time.
fn platform_defaults_to_one_thread() -> bool {
    cfg!(target_family = "wasm")
}

#[derive(Debug, Clone)]
struct FilterIgnored {
    args: Arguments,
}

impl Filter for FilterIgnored {
    fn should_run(&self, test: &'static TestCase, _info: &TestRunInfo) -> bool {
        !test.tags.contains(&IGNORE_TAG_NAME) || self.args.ignored || self.args.include_ignored
    }
}

const DEPTH_STEP: &str = "> ";

fn print_list(tree: &TestsTree, ignored: bool, depth: usize) {
    // * all tests without `--ignored`
    // * just the ignored tests with `--ignored`
    if !ignored || tree.node.tags.contains(&IGNORE_TAG_NAME) {
        if !tree.node.tags.is_empty() {
            println!(
                "{}[{}] {}",
                DEPTH_STEP.repeat(depth),
                tree.node.tags.join(", "),
                tree.node.name
            );
        } else {
            println!("{}{}", DEPTH_STEP.repeat(depth), tree.node.name);
        };
    }

    for tree in &tree.childs {
        print_list(tree, ignored, depth + 1);
    }
}

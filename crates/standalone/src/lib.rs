//! The default test runner for the [testscribe](https://docs.rs/testscribe) test framework.
//!
//! It integrates testscribe's test trees with the standard test harness, so a whole tree runs
//! under plain `cargo test`. Most users don't depend on this crate directly: it is pulled in
//! automatically by the main [`testscribe`](https://docs.rs/testscribe) crate through its default
//! `standalone` feature.
//!
//! You only need this crate directly when building a **custom test runner** in a `harness = false`
//! integration test. [`run_all_sync`] is the entry point: it parses libtest-style arguments,
//! filters, runs the whole tree, and reports results.
//!
//! ```rust,ignore
//! use std::process::ExitCode;
//!
//! use testscribe::CASES;
//! use testscribe::standalone::{args::Arguments, run_all_sync};
//!
//! fn main() -> ExitCode {
//!     run_all_sync(&CASES, Arguments::from_args())
//!         .unwrap()
//!         .exit_code()
//! }
//! ```
//!
//! See the `custom_test_runner` example in the main crate for a complete setup.

#[doc(hidden)]
pub mod args;
#[doc(hidden)]
pub mod logger;
#[doc(hidden)]
pub mod panic_hook;

mod filter;
mod runner;

use std::{collections::HashSet, future::Future, io::Write, sync::Mutex, time::Duration};

use futures::executor::block_on;

use testscribe_core::{
    processor::{
        filter::{Filter, FilterByPath, NoFilter},
        logger::{Logger, TestRunInfo, TestStatusUpdate},
    },
    test_case::{FqFnName, TestCase},
    tests_tree::{
        AugmentedTestsTree, BuildTreeError, TestsTree, create_test_trees, filter_test_trees,
    },
};

use crate::{
    args::Arguments,
    filter::{IGNORE_TAG_NAME, filter_out_test},
    logger::{printer::TestFormatter, summary::ExecutionSummary},
};

pub use crate::runner::run_test_tree;

/// Used for development with "standalone" tests, to run only a specific test node in a test tree.
/// This tag is ignored by default, but if `--exact` flag is used, only tests with this tag will be run.
pub const ONLY_THIS_TAG_NAME: &str = "exact_this";

/// Names the tests to run in standalone trees, without touching the source:
/// `TESTSCRIBE_ONLY=order_shipped cargo test` runs the path from the root to `order_shipped`
/// and nothing else. Several names are separated by commas. A name is the function name of the
/// test, or the name of its marker type (`OrderShipped`).
///
/// The harness only knows root tests, so naming a test below the root to `cargo test` itself
/// runs nothing. A tree without any of the named tests is not run.
pub const ONLY_ENV_NAME: &str = "TESTSCRIBE_ONLY";

/// Whether `test_name`, the marker type of a test, is one of the comma-separated names in `only`.
fn is_named(only: &str, test_name: &str) -> bool {
    only.split(',').map(str::trim).any(|name| {
        let marker_type: String = name
            .split('_')
            .filter(|word| !word.is_empty())
            .flat_map(|word| {
                let (first, rest) = word.split_at(1);
                [first.to_ascii_uppercase(), rest.to_string()]
            })
            .collect();
        test_name == name || test_name == marker_type
    })
}

/// The filter that runs the wanted tests of `tree` and the tests above them, or `None` if the
/// tree has no wanted test.
fn paths_to(tree: &TestsTree, wanted: impl Fn(&'static TestCase) -> bool) -> Option<FilterByPath> {
    let mut marked = AugmentedTestsTree::new(tree, wanted);
    marked.update_parents(|parent, child| {
        parent.extra |= child.extra;
    });
    if !marked.node.extra {
        return None;
    }
    let mut paths = vec![];
    marked.visit_breath(|node, depth| {
        if node.extra {
            if paths.len() <= depth {
                paths.push(HashSet::new());
            }
            paths[depth].insert(node.test.name);
        }
    });
    Some(FilterByPath { paths })
}

/// This function signature is used by proc-macros to run `standalone` tests synchronously.
pub fn run_sync(
    test_cases: &'static [TestCase],
    module_path: &'static str,
    test_name: &'static str,
) -> Result<(), String> {
    block_on(run_async(test_cases, module_path, test_name))
}

/// Which tests of a tree to run.
pub enum Selection {
    /// The whole tree.
    All,
    /// The tests on these paths, for every path a list of names from the root down.
    Paths(Vec<Vec<String>>),
    /// Nothing.
    None,
}

/// Something that follows a standalone tree from outside, such as a remote frontend: it may
/// choose which tests run, sees every status update, and is told when the tree is done.
pub trait TreeObserver {
    /// Asked once, before the tree runs. A selection made on the command line, with
    /// `--exact` or [`ONLY_ENV_NAME`], comes first: then this is not asked.
    fn select(&mut self, tree: &TestsTree) -> impl Future<Output = Selection>;

    /// Every status update of the tree, before it is printed.
    fn observe(&mut self, test: &'static TestCase, update: &TestStatusUpdate, elapsed: Duration);

    /// The tree is done. Returns when the observer has passed everything on.
    fn finished(&mut self, success: bool) -> impl Future<Output = ()>;
}

/// Nobody is watching.
pub struct NoObserver;

impl TreeObserver for NoObserver {
    async fn select(&mut self, _tree: &TestsTree) -> Selection {
        Selection::All
    }

    fn observe(&mut self, _: &'static TestCase, _: &TestStatusUpdate, _: Duration) {}

    async fn finished(&mut self, _success: bool) {}
}

/// Shows every status update to an observer on its way to the printer.
struct Observed<'a, O, P> {
    observer: &'a mut O,
    printer: &'a mut P,
}

impl<O: TreeObserver, P: Logger> Logger for Observed<'_, O, P> {
    fn log(&mut self, test: &'static TestCase, update: TestStatusUpdate, elapsed: Duration) {
        self.observer.observe(test, &update, elapsed);
        self.printer.log(test, update, elapsed);
    }
}

/// The filter for the tests of `tree` on the named paths.
fn named_paths(tree: &TestsTree, named: &[Vec<String>]) -> FilterByPath {
    let mut paths: Vec<HashSet<FqFnName<'static>>> = Vec::new();
    let mut level = vec![(tree, 0)];
    while let Some((node, depth)) = level.pop() {
        let name = node.node.name;
        let on_a_path = named.iter().any(|path| {
            path.get(depth)
                .is_some_and(|wanted| *wanted == name.to_string())
        });
        if on_a_path {
            if paths.len() <= depth {
                paths.resize_with(depth + 1, HashSet::new);
            }
            paths[depth].insert(name);
            level.extend(node.childs.iter().map(|child| (child, depth + 1)));
        }
    }
    FilterByPath { paths }
}

/// This function signature is used by proc-macros to run `standalone` tests asynchronously.
pub async fn run_async(
    test_cases: &'static [TestCase],
    module_path: &'static str,
    test_name: &'static str,
) -> Result<(), String> {
    run_observed(test_cases, module_path, test_name, &mut NoObserver).await
}

/// Runs a standalone tree the way [`run_async`] does, in front of an observer.
pub async fn run_observed<O: TreeObserver>(
    test_cases: &'static [TestCase],
    module_path: &'static str,
    test_name: &'static str,
    observer: &mut O,
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
    let live = prints_live(&args, std::env::var("RUST_TEST_THREADS").ok().as_deref());
    let mut stdout = std::io::stdout();
    let mut vecout = Vec::new();
    let output: &mut dyn Write = if live { &mut stdout } else { &mut vecout };
    let paths = if let Ok(only) = std::env::var(ONLY_ENV_NAME) {
        let Some(paths) = paths_to(&tree, |test| is_named(&only, test.name.name)) else {
            // None of the named tests is in this tree, so it was not asked for.
            return Ok(());
        };
        Some(paths)
    } else if args.exact {
        paths_to(&tree, |test| test.tags.contains(&ONLY_THIS_TAG_NAME))
    } else {
        None
    };
    let paths = match paths {
        Some(paths) => Some(paths),
        None => match observer.select(&tree).await {
            Selection::All => None,
            Selection::Paths(named) => Some(named_paths(&tree, &named)),
            Selection::None => {
                observer.finished(true).await;
                return Ok(());
            }
        },
    };
    let filter: &dyn Filter = match &paths {
        Some(paths) => paths,
        None => &NoFilter,
    };

    let mut printer = TestFormatter::new(output);
    let summary = {
        let mut observed = Observed {
            observer: &mut *observer,
            printer: &mut printer,
        };
        run_test_tree(tree, filter, &mut observed, args.exact).await
    };
    printer.print_failures(&summary.failed);
    printer.print_panics(&summary.panics);
    if !live {
        // In one piece: other trees print at the same time.
        print!("{}", String::from_utf8(vecout).unwrap());
    }
    observer.finished(summary.is_success()).await;
    if summary.is_success() {
        Ok(())
    } else {
        Err(format!("Test {test_name} failed."))
    }
}

/// Whether a standalone tree prints each test as it finishes, or the whole tree at once when
/// it is done.
///
/// The harness runs standalone trees in parallel, and with `--nocapture` their lines would mix
/// into something nobody can read. So a tree prints live only when it is certain to be alone:
/// with `--nocapture` and one test thread.
fn prints_live(args: &Arguments, rust_test_threads: Option<&str>) -> bool {
    let test_threads = args
        .test_threads
        .or_else(|| rust_test_threads.and_then(|threads| threads.parse().ok()));
    args.nocapture && test_threads == Some(1)
}

/// Runs tests similarly to `libtest`,
/// However since we're running test trees, some behaviour is different:
///
pub fn run_all_sync(
    test_cases: &'static [TestCase],
    args: Arguments,
) -> Result<ExecutionSummary, Box<BuildTreeError>> {
    let trees = filter_test_trees(create_test_trees(test_cases), |test| {
        filter_out_test(test, &args)
    });
    trees.iter().try_for_each(|tree| tree.verify(false))?;

    if args.list {
        for tree in &trees {
            print_list(tree, args.ignored, 0);
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
        let iter = receiver.try_iter().take(num_roots);
        for (test_summary, vecout) in iter {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn args(nocapture: bool, test_threads: Option<usize>) -> Arguments {
        Arguments {
            nocapture,
            test_threads,
            ..Default::default()
        }
    }

    #[test]
    fn test_is_named_by_its_function_or_by_its_marker_type() {
        assert!(is_named("order_shipped", "OrderShipped"));
        assert!(is_named("OrderShipped", "OrderShipped"));
        assert!(is_named("order_paid, order_shipped", "OrderShipped"));
        assert!(is_named("another_6_is_added", "Another6IsAdded"));
        // A name is the whole name: a part of it names nothing.
        assert!(!is_named("order", "OrderShipped"));
        assert!(!is_named("order_shipped_twice", "OrderShipped"));
        assert!(!is_named("", "OrderShipped"));
    }

    #[test]
    fn standalone_tree_prints_live_only_when_it_runs_alone() {
        // Captured output is shown by the harness, per test, when the test fails.
        assert!(!prints_live(&args(false, None), None));
        assert!(!prints_live(&args(false, Some(1)), None));
        // Not captured, next to other trees: in one piece, so that the trees do not mix.
        assert!(!prints_live(&args(true, None), None));
        assert!(!prints_live(&args(true, Some(4)), None));
        assert!(!prints_live(&args(true, None), Some("4")));
        // Not captured and alone: as it happens.
        assert!(prints_live(&args(true, Some(1)), None));
        assert!(prints_live(&args(true, None), Some("1")));
        // The argument wins over the environment, as it does in the harness.
        assert!(!prints_live(&args(true, Some(4)), Some("1")));
    }
}

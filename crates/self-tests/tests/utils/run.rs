use testscribe::processor::filter::NoFilter;
use testscribe::standalone::logger::printer::TestFormatter;
use testscribe::standalone::logger::summary::ExecutionSummary;
use testscribe::standalone::run_test_tree;
use testscribe::test_case::TestCase;

use super::tree::create_and_verify_tt;

pub struct Run {
    pub summary: ExecutionSummary,
    pub output: String,
}

impl Run {
    /// The tests that failed, each with the details of its failures.
    pub fn failed(&self) -> Vec<(&str, Vec<&str>)> {
        self.summary
            .failed
            .iter()
            .map(|(name, failures)| {
                (
                    name.name,
                    failures.iter().map(|f| f.details.as_str()).collect(),
                )
            })
            .collect()
    }

    /// The printed lines naming a test, without timings and colors: the status mark and the name.
    pub fn narrative(&self) -> Vec<String> {
        self.output
            .lines()
            .filter(|line| line.contains("Given") || line.contains("When"))
            .map(|line| {
                let mark = &line[..1];
                let name = line.rsplit("\u{1b}[0m ").next().unwrap();
                let name = name.rsplit("Given ").next().unwrap();
                let name = name.rsplit("When ").next().unwrap();
                format!("{mark} {name}")
            })
            .collect()
    }
}

/// Runs the tree whose root test is `root`, the way a standalone test would, and tells what
/// came of it.
pub async fn run_tree(cases: &'static [TestCase], root: &str) -> Run {
    let tree = create_and_verify_tt(cases, false)
        .unwrap()
        .into_iter()
        .find(|tree| tree.node.name.name == root)
        .unwrap();
    let mut output = Vec::new();
    let mut printer = TestFormatter::new(&mut output);
    let summary = run_test_tree(tree, &NoFilter, &mut printer, false).await;
    Run {
        summary,
        output: String::from_utf8(output).unwrap(),
    }
}

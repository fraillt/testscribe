//! What a test said, as text: the narrative the terminal prints, without the times, and
//! without where in the sources a test or a check is. Two runs of a test that read the same
//! are the same outcome, whatever was moved around in the code between them.

use std::collections::{HashMap, HashSet};

use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Passed,
    Failed,
    /// The test could not run, because a test above it panicked.
    Skipped,
}

impl Status {
    pub fn as_str(self) -> &'static str {
        match self {
            Status::Passed => "passed",
            Status::Failed => "failed",
            Status::Skipped => "skipped",
        }
    }

    pub fn parse(text: &str) -> Status {
        match text {
            "passed" => Status::Passed,
            "skipped" => Status::Skipped,
            _ => Status::Failed,
        }
    }
}

/// A test is known by its place in the tree: the names from the root down to it, each one
/// as `module::path::TestName`.
pub type TestPath = Vec<String>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestOutcome {
    pub path: TestPath,
    pub status: Status,
    pub text: String,
}

/// One execution of a test, while it is going on.
struct Run {
    path: TestPath,
    lines: Vec<String>,
    /// Checks and tables so far: the first reads `Then`, the others `And`.
    items: usize,
    failed: bool,
    again: bool,
}

/// Follows the tests of one tree as they report, and keeps the outcome of each test that
/// ran. A test that was left out of the run is not in it.
#[derive(Default)]
pub struct TreeOutcome {
    tests: Vec<TestOutcome>,
    index: HashMap<TestPath, usize>,
    current: Option<Run>,
    told_as_failed: HashSet<TestPath>,
}

fn full_name(name: &Value) -> String {
    format!(
        "{}::{}",
        name["path"].as_str().unwrap_or_default(),
        name["name"].as_str().unwrap_or_default()
    )
}

/// `module::Another6IsAdded` reads `another 6 is added`, as in the terminal.
pub fn title(full_name: &str) -> String {
    let name = full_name.rsplit("::").next().unwrap_or(full_name);
    let mut title = String::with_capacity(name.len() + 8);
    let mut after_digit = false;
    for (idx, letter) in name.chars().enumerate() {
        let new_word = letter.is_ascii_uppercase() || (letter.is_ascii_digit() && !after_digit);
        if idx > 0 && new_word {
            title.push(' ');
        }
        title.push(letter.to_ascii_lowercase());
        after_digit = letter.is_ascii_digit();
    }
    title
}

fn test_path(test: &Value, info: &Value) -> TestPath {
    info["path"]
        .as_array()
        .into_iter()
        .flatten()
        .chain(Some(test))
        .map(full_name)
        .collect()
}

fn heading(path: &TestPath) -> String {
    let keyword = if path.len() == 1 { "Given" } else { "When" };
    format!(
        "{keyword} {}",
        title(path.last().map_or("", String::as_str))
    )
}

fn texts(list: &Value) -> Vec<&str> {
    list.as_array()
        .into_iter()
        .flatten()
        .map(|text| text.as_str().unwrap_or_default())
        .collect()
}

/// A line with the mark of a failure in front of it, and what follows it set below.
fn marked(failed: bool, text: &str, lines: &mut Vec<String>) {
    let mut parts = text.lines();
    let mark = if failed { "✗" } else { " " };
    lines.push(format!("{mark} {}", parts.next().unwrap_or_default()));
    lines.extend(parts.map(|part| format!("      {part}")));
}

fn failure(outcome: &Value) -> Option<&str> {
    (outcome["kind"] != "Success").then(|| outcome["details"].as_str().unwrap_or_default())
}

impl TreeOutcome {
    /// Takes a `test` message of the tree.
    pub fn apply(&mut self, event: &Value) {
        let update = &event["update"];
        match update["kind"].as_str() {
            Some("Started") => {
                let info = &update["info"];
                let path = test_path(&event["test"], info);
                let again = info["run_count"].as_u64().unwrap_or(0) > 0;
                let mut lines = vec![heading(&path)];
                if again {
                    lines[0].push_str(" (executed again)");
                }
                let param = &info["param_info"];
                if !param.is_null() {
                    let pairs: Vec<String> = texts(&param["headers"])
                        .iter()
                        .zip(texts(&param["display_str"]))
                        .map(|(header, value)| format!("{header}={value}"))
                        .collect();
                    lines.push(format!("With {}", pairs.join(", ")));
                }
                self.current = Some(Run {
                    path,
                    lines,
                    items: 0,
                    failed: false,
                    again,
                });
            }
            Some("Updated") => {
                let Some(run) = &mut self.current else { return };
                let info = &update["info"];
                let keyword = if run.items == 0 { "Then" } else { "And" };
                let message = info["message"].as_str().unwrap_or_default();
                match info["kind"].as_str() {
                    Some("Verified") => {
                        run.items += 1;
                        let failure = failure(&info["outcome"]);
                        run.failed |= failure.is_some();
                        marked(
                            failure.is_some(),
                            &format!("{keyword} {message}"),
                            &mut run.lines,
                        );
                        if let Some(details) = failure {
                            run.lines
                                .extend(details.lines().map(|line| format!("      {line}")));
                        }
                    }
                    Some("ParamsStarted") => {
                        run.items += 1;
                        marked(false, &format!("{keyword} {message}"), &mut run.lines);
                        if !info["header"].is_null() {
                            let header = texts(&info["header"]).join(" | ");
                            run.lines.push(format!("    | {header} |"));
                        }
                    }
                    Some("ParamVerified") => {
                        let row = texts(&info["row"]).join(" | ");
                        match failure(&info["outcome"]) {
                            Some(details) => {
                                run.failed = true;
                                marked(true, &format!("  | {row} |  {details}"), &mut run.lines);
                            }
                            None => run.lines.push(format!("    | {row} |")),
                        }
                    }
                    _ => {}
                }
            }
            Some("Finished") => {
                let Some(mut run) = self.current.take() else {
                    return;
                };
                if let Some(panic) = update["panic_message"].as_str() {
                    run.failed = true;
                    marked(true, &format!("panicked: {panic}"), &mut run.lines);
                }
                // A test is told for its first run. A run that executes it again for the
                // tests below it is told only if it fails where no earlier run did.
                let first_failure = run.failed && self.told_as_failed.insert(run.path.clone());
                if !run.again || first_failure {
                    let status = if run.failed {
                        Status::Failed
                    } else {
                        Status::Passed
                    };
                    self.tell(run.path, status, run.lines.join("\n"));
                }
            }
            Some("Skipped") => {
                let info = &update["info"];
                let reason = &update["reason"];
                // A test that was not selected did not run: nothing is known of it.
                if reason["kind"] != "Panicked" || info["run_count"].as_u64().unwrap_or(0) > 0 {
                    return;
                }
                let path = test_path(&event["test"], info);
                let message = reason["message"].as_str().unwrap_or_default();
                let mut lines = vec![heading(&path)];
                lines.push(format!("– skipped: {}", message.replace('\n', "\n      ")));
                self.tell(path, Status::Skipped, lines.join("\n"));
            }
            _ => {}
        }
    }

    /// A test with parameters above it runs once for each: its outcome is all of its runs.
    fn tell(&mut self, path: TestPath, status: Status, text: String) {
        match self.index.get(&path) {
            Some(&idx) => {
                let test = &mut self.tests[idx];
                test.text.push_str("\n\n");
                test.text.push_str(&text);
                test.status = match (test.status, status) {
                    (Status::Failed, _) | (_, Status::Failed) => Status::Failed,
                    (Status::Skipped, _) | (_, Status::Skipped) => Status::Skipped,
                    _ => Status::Passed,
                };
            }
            None => {
                self.index.insert(path.clone(), self.tests.len());
                self.tests.push(TestOutcome { path, status, text });
            }
        }
    }

    /// The tests that ran so far, in the order they first ran.
    pub fn tests(&self) -> &[TestOutcome] {
        &self.tests
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn name(name: &str) -> Value {
        json!({ "path": "orders", "name": name })
    }

    fn event(test: &str, update: Value) -> Value {
        json!({ "kind": "test", "root": "orders::OrderPlaced", "test": name(test), "update": update, "elapsed_us": 5 })
    }

    fn started(test: &str, above: &[&str], run_count: u32) -> Value {
        let path: Vec<Value> = above.iter().map(|above| name(above)).collect();
        event(
            test,
            json!({ "kind": "Started", "info": { "path": path, "run_count": run_count, "param_info": null } }),
        )
    }

    fn verified(test: &str, message: &str, line_nr: u32, details: Option<&str>) -> Value {
        let outcome = match details {
            Some(details) => json!({ "kind": "Failure", "details": details }),
            None => json!({ "kind": "Success" }),
        };
        event(
            test,
            json!({ "kind": "Updated", "info": {
                "kind": "Verified", "message": message, "line_nr": line_nr, "file": "tests/orders.rs", "outcome": outcome,
            } }),
        )
    }

    fn finished(test: &str, panic: Option<&str>) -> Value {
        event(test, json!({ "kind": "Finished", "panic_message": panic }))
    }

    fn outcome_of(events: &[Value]) -> Vec<TestOutcome> {
        let mut tree = TreeOutcome::default();
        events.iter().for_each(|event| tree.apply(event));
        tree.tests().to_vec()
    }

    #[test]
    fn title_reads_as_in_the_terminal() {
        assert_eq!(title("orders::Another6IsAdded"), "another 6 is added");
        assert_eq!(title("orders::Order42Paid"), "order 42 paid");
    }

    #[test]
    fn outcome_is_the_narrative_of_the_test() {
        let tests = outcome_of(&[
            started("OrderPlaced", &[], 0),
            finished("OrderPlaced", None),
            started("OrderPaid", &["OrderPlaced"], 0),
            verified("OrderPaid", "order number equals 17", 20, None),
            verified(
                "OrderPaid",
                "total equals 5",
                21,
                Some("expected: 5\nactual: 6"),
            ),
            finished("OrderPaid", None),
        ]);
        assert_eq!(
            tests,
            vec![
                TestOutcome {
                    path: vec!["orders::OrderPlaced".into()],
                    status: Status::Passed,
                    text: "Given order placed".into(),
                },
                TestOutcome {
                    path: vec!["orders::OrderPlaced".into(), "orders::OrderPaid".into()],
                    status: Status::Failed,
                    text: "When order paid\n  Then order number equals 17\n✗ And total equals 5\n      expected: 5\n      actual: 6".into(),
                },
            ]
        );
    }

    #[test]
    fn where_a_check_is_in_the_sources_is_not_part_of_the_outcome() {
        let at = |line_nr| {
            outcome_of(&[
                started("OrderPlaced", &[], 0),
                verified("OrderPlaced", "order is new", line_nr, None),
                finished("OrderPlaced", None),
            ])
        };
        assert_eq!(at(20), at(87));
    }

    #[test]
    fn test_executed_again_is_told_only_when_it_fails_for_the_first_time() {
        let tests = outcome_of(&[
            started("OrderPlaced", &[], 0),
            finished("OrderPlaced", None),
            started("OrderPlaced", &[], 1),
            finished("OrderPlaced", None),
            started("OrderPlaced", &[], 2),
            finished("OrderPlaced", Some("no connection")),
            started("OrderPlaced", &[], 3),
            finished("OrderPlaced", Some("no connection")),
        ]);
        assert_eq!(tests.len(), 1);
        assert_eq!(tests[0].status, Status::Failed);
        assert_eq!(
            tests[0].text,
            "Given order placed\n\nGiven order placed (executed again)\n✗ panicked: no connection"
        );
    }

    #[test]
    fn test_left_out_of_the_run_has_no_outcome_and_one_that_could_not_run_is_skipped() {
        let skipped = |test: &str, reason: Value| {
            event(
                test,
                json!({ "kind": "Skipped", "info": { "path": [name("OrderPlaced")], "run_count": 0, "param_info": null }, "reason": reason }),
            )
        };
        let tests = outcome_of(&[
            skipped(
                "OrderPaid",
                json!({ "kind": "Ignored", "name": name("OrderPaid") }),
            ),
            skipped(
                "OrderShipped",
                json!({ "kind": "Panicked", "name": name("OrderPlaced"), "location": { "kind": "Test" }, "message": "no connection" }),
            ),
        ]);
        assert_eq!(
            tests,
            vec![TestOutcome {
                path: vec!["orders::OrderPlaced".into(), "orders::OrderShipped".into()],
                status: Status::Skipped,
                text: "When order shipped\n– skipped: no connection".into(),
            }]
        );
    }
}

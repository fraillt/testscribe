use std::collections::HashMap;
use std::io::Write;
use std::mem::take;
use std::time::Duration;

use colored::Colorize;

use testscribe_core::processor::logger::{
    TestRunInfo, TestStatusUpdate, TestUpdate, VerifyOutcome,
};
use testscribe_core::test_case::{FqFnName, TestCase};
use testscribe_core::tests_tree::TestsTree;

use crate::logger::{FailedTest, Failure};

pub struct TestFormatter<'a> {
    test_info: HashMap<FqFnName<'static>, &'static TestCase>,
    out: &'a mut dyn Write,
    current_test: Option<TestRunInfo>,
    started_at: Option<Duration>,
    test_updates: Vec<TestUpdate>,
    pub failed_tests: Vec<FailedTest>,
    test_with_params: Option<FqFnName<'static>>,
}

impl<'a> TestFormatter<'a> {
    pub fn new(dag: &TestsTree, out: &'a mut dyn Write) -> Self {
        let mut test_info = HashMap::new();
        dag.visit(&mut |t| {
            test_info.insert(t.name, t);
        });
        Self {
            current_test: None,
            started_at: None,
            test_updates: Vec::new(),
            test_info,
            failed_tests: Default::default(),
            out,
            test_with_params: None,
        }
    }

    pub fn replay_event(&mut self, update: TestStatusUpdate, elapsed: Duration) {
        match update {
            TestStatusUpdate::Started { info } => {
                self.current_test = Some(info);
                self.started_at = Some(elapsed)
            }
            TestStatusUpdate::Updated { info } => {
                self.test_updates.push(info);
            }
            TestStatusUpdate::Finished { panic_message } => {
                self.finished(panic_message, elapsed);
            }
            TestStatusUpdate::Skipped { info, reason: _ } => {
                if info.run_count == 0 {
                    writeln!(
                        self.out,
                        "?|{: >8}|{}{} {}",
                        format_time(Duration::from_secs(0)),
                        "  ".repeat(info.depth),
                        if info.depth == 0 {
                            "Given".yellow()
                        } else {
                            "When".yellow()
                        },
                        make_test_name(info.name.name)
                    )
                    .unwrap();
                }
            }
        }
    }

    fn finished(&mut self, panic_message: Option<String>, elapsed: Duration) {
        let test = self.current_test.clone().unwrap();
        let mut failures = Vec::new();
        if let Some(payload) = &panic_message {
            let info = self.test_info.get(&test.name).unwrap();
            failures.push(Failure {
                param_index: None,
                message: "test should not panic".to_string(),
                file: info.filename,
                line_nr: info.line_nr,
                details: payload.clone(),
            });
        }
        let duration = elapsed - self.started_at.unwrap();
        let test_updates = take(&mut self.test_updates);
        if test.run_count == 0 {
            if Some(test.name) != self.test_with_params {
                writeln!(
                    self.out,
                    "{}|{: >8}|{}{} {}",
                    if panic_message.is_some() { "!" } else { " " },
                    format_time(duration),
                    "  ".repeat(test.depth),
                    if test.depth == 0 {
                        "Given".yellow()
                    } else {
                        "When".yellow()
                    },
                    make_test_name(test.name.name)
                )
                .unwrap();
            }
            if let Some(param) = &test.param_info {
                self.test_with_params = Some(test.name);
                writeln!(
                    self.out,
                    " |        |{}- with {}",
                    "  ".repeat(test.depth),
                    param
                        .headers
                        .iter()
                        .zip(param.display_str.iter())
                        .map(|(header, label)| format!("{}={}", header, label))
                        .collect::<Vec<String>>()
                        .join(","),
                )
                .unwrap();
            } else {
                self.test_with_params = None;
            }

            let mut params_state = None;
            for (index, update) in test_updates.into_iter().enumerate() {
                match update {
                    TestUpdate::Verified {
                        message,
                        line_nr,
                        file,
                        outcome,
                    } => {
                        writeln!(
                            self.out,
                            "{}|       -|  {}{} {}",
                            get_assertion_status(&outcome),
                            "  ".repeat(test.depth),
                            if index == 0 {
                                if let VerifyOutcome::Success = &outcome {
                                    "Then".yellow()
                                } else {
                                    "Then".red()
                                }
                            } else {
                                if let VerifyOutcome::Success = &outcome {
                                    "And".yellow()
                                } else {
                                    "And".red()
                                }
                            },
                            if let VerifyOutcome::Success = &outcome {
                                message.white()
                            } else {
                                message.red()
                            }
                        )
                        .unwrap();
                        if let VerifyOutcome::Failure { details } = outcome {
                            failures.push(Failure {
                                param_index: None,
                                message,
                                line_nr,
                                file,
                                details,
                            });
                        }
                    }
                    TestUpdate::ParamsStarted {
                        message,
                        line_nr,
                        file,
                        header,
                    } => {
                        params_state = Some(ParamsState {
                            message,
                            line_nr,
                            file,
                            header,
                            outcomes: Default::default(),
                            rows_fields: Default::default(),
                        });
                    }
                    TestUpdate::ParamVerified {
                        row_fields,
                        outcome,
                    } => {
                        let state = params_state.as_mut().unwrap();
                        state.rows_fields.push(row_fields);
                        state.outcomes.push(outcome);
                    }
                    TestUpdate::ParamsFinished => {
                        let mut state = params_state.take().unwrap();

                        let header =
                            format_table_header_and_rows(&state.header, &mut state.rows_fields);
                        writeln!(
                            self.out,
                            " |       -|  {}{} {}",
                            "  ".repeat(test.depth),
                            if index == 0 {
                                "Then".yellow()
                            } else {
                                "And".yellow()
                            },
                            &state.message,
                        )
                        .unwrap();
                        writeln!(
                            self.out,
                            " |       -|  {}|{} |",
                            "  ".repeat(test.depth),
                            header.join(",")
                        )
                        .unwrap();

                        for (index, (row, outcome)) in
                            state.rows_fields.iter().zip(state.outcomes).enumerate()
                        {
                            writeln!(
                                self.out,
                                "{}|       -|  {}|{} |",
                                get_assertion_status(&outcome),
                                "  ".repeat(test.depth),
                                if let VerifyOutcome::Success = &outcome {
                                    row.join(",")
                                } else {
                                    row.join(",").red().to_string()
                                },
                            )
                            .unwrap();
                            if let VerifyOutcome::Failure { details } = outcome {
                                failures.push(Failure {
                                    param_index: Some(index),
                                    message: state.message.clone(),
                                    line_nr: state.line_nr,
                                    file: state.file,
                                    details,
                                });
                            }
                        }
                    }
                };
            }
        }

        if !failures.is_empty() {
            self.failed_tests.push(FailedTest {
                name: test.name,
                failures,
            });
        }
    }
}

struct ParamsState {
    message: String,
    line_nr: u32,
    file: &'static str,
    header: Vec<&'static str>,
    rows_fields: Vec<Vec<String>>,
    outcomes: Vec<VerifyOutcome>,
}

// returns formatted header
fn format_table_header_and_rows(
    header: &[&'static str],
    rows: &mut Vec<Vec<String>>,
) -> Vec<String> {
    let mut res_header = Vec::from_iter(header.into_iter().map(|h| h.to_string()));
    for (i, h) in res_header.iter_mut().enumerate() {
        let mut max_size = h.len();
        for r in rows.iter_mut() {
            if max_size < r[i].len() {
                max_size = r[i].len();
            }
        }
        max_size += 1;
        // add spaces
        // h.extend(" ".chars().cycle().take(max_size - h.len()));
        h.insert_str(
            0,
            &" ".chars()
                .cycle()
                .take(max_size - h.len())
                .collect::<String>(),
        );
        for r in rows.iter_mut() {
            let curr_len = r[i].len();
            r[i].insert_str(
                0,
                &" ".chars()
                    .cycle()
                    .take(max_size - curr_len)
                    .collect::<String>(),
            );
            // r[i].extend(" ".chars().cycle().take(max_size - curr_len))
        }
    }
    res_header
}

fn get_assertion_status(outcome: &VerifyOutcome) -> &'static str {
    if let VerifyOutcome::Success = outcome {
        " "
    } else {
        "!"
    }
}
fn make_test_name(name: &str) -> String {
    let mut res = Vec::with_capacity(name.len() + 10);
    res.push(name.as_bytes()[0].to_ascii_lowercase());
    let mut prev_is_digit = false;
    for b in name.as_bytes().into_iter().skip(1) {
        if b.is_ascii_uppercase() || (b.is_ascii_digit() && !prev_is_digit) {
            res.push(b' ');
            res.push(b.to_ascii_lowercase());
        } else {
            res.push(*b)
        };
        prev_is_digit = b.is_ascii_digit()
    }
    unsafe { String::from_utf8_unchecked(res) }
}

fn format_time(time: Duration) -> String {
    let mut time = time.as_nanos();
    const UNITS: [&str; 4] = ["ns", "μs", "ms", "s"];
    let mut unit_index = 0;
    let mut fraction = 0;
    while time >= 10 && unit_index < (UNITS.len() - 1) {
        fraction = time % 1000;
        time /= 1000;
        unit_index += 1;
    }
    if time >= 10 {
        format!("{:0>2}:{:0>2}", time / 60, time % 60)
    } else {
        format!("{time}.{:0>3}{}", fraction, UNITS[unit_index])
    }
}

#[test]
fn boo() {
    assert_eq!(format_time(Duration::from_secs(1812)), "30:12");
    assert_eq!(format_time(Duration::from_secs(561)), "09:21");
    assert_eq!(format_time(Duration::from_millis(10012)), "00:10");
    assert_eq!(format_time(Duration::from_millis(9012)), "9.012s");
    assert_eq!(format_time(Duration::from_millis(1590)), "1.590s");
    assert_eq!(format_time(Duration::from_millis(12)), "0.012s");
    assert_eq!(format_time(Duration::from_micros(10000)), "0.010s");
    assert_eq!(format_time(Duration::from_micros(9999)), "9.999ms");
    assert_eq!(format_time(Duration::from_nanos(8)), "8.000ns");
}

use std::io::Write;
use std::mem::take;
use std::path::Path;
use std::time::Duration;

use backtrace::BacktraceFrame;
use colored::Colorize;

use testscribe_core::processor::logger::{
    Logger, SkipReason, TestRunInfo, TestStatusUpdate, TestUpdate, VerifyOutcome,
};
use testscribe_core::test_case::{FqFnName, TestCase};

use crate::logger::summary::Failure;
use crate::panic_hook::PanicDetails;

pub struct TestFormatter<'a> {
    out: &'a mut dyn Write,
    current_test_info: Option<TestRunInfo>,
    started_at: Option<Duration>,
    test_updates: Vec<TestUpdate>,
}

impl Logger for TestFormatter<'_> {
    fn log(&mut self, test: &'static TestCase, update: TestStatusUpdate, elapsed: Duration) {
        match update {
            TestStatusUpdate::Started { info } => {
                self.current_test_info = Some(info);
                self.started_at = Some(elapsed)
            }
            TestStatusUpdate::Updated { info } => {
                self.test_updates.push(info);
            }
            TestStatusUpdate::Finished { panic_message } => {
                self.finished(test, panic_message, elapsed);
            }
            TestStatusUpdate::Skipped { info, reason } => {
                self.skipped(test.name, info, reason);
            }
        }
    }
}

impl<'a> TestFormatter<'a> {
    pub fn new(out: &'a mut dyn Write) -> Self {
        Self {
            current_test_info: None,
            started_at: None,
            test_updates: Vec::new(),
            out,
        }
    }

    pub fn print_panics(&mut self, panics: &[PanicDetails]) {
        if !panics.is_empty() {
            writeln!(self.out, "panic info:").unwrap();
        }

        for panic in panics {
            writeln!(
                self.out,
                "loc | {}:{}:{}",
                panic.location.file, panic.location.line, panic.location.col
            )
            .unwrap();
            let msg = panic
                .message
                .clone()
                .map(|msg| msg.replace('\n', "\n    | "))
                .unwrap_or_else(|| "???".to_owned());
            writeln!(self.out, "msg | {msg}",).unwrap();
            let mut bt = panic.backtrace.clone();
            writeln!(self.out, "bt  |").unwrap();
            // resolve everything is very slow
            bt.resolve();
            let frames: Vec<BacktraceFrame> = bt.into();
            for (index, symbol) in frames
                .iter()
                .flat_map(|f| f.symbols())
                .filter_map(|s| {
                    Some(BtSymbol {
                        name: s.name()?.to_string(),
                        path: s.filename()?,
                        lineno: s.lineno()?,
                        colno: s.colno()?,
                    })
                })
                .skip_while(|s| s.name != "rust_begin_unwind")
                .take_while(|s| !s.name.contains("::TEST_CASE_"))
                .enumerate()
            {
                writeln!(self.out, "{:>3} | {}", index + 1, symbol.name).unwrap();
                writeln!(
                    self.out,
                    "    | {}:{}:{}",
                    symbol.path.to_string_lossy(),
                    symbol.lineno,
                    symbol.colno
                )
                .unwrap();
            }
        }
    }

    pub fn print_failures(&mut self, failed: &[(FqFnName<'static>, Vec<Failure>)]) {
        if !failed.is_empty() {
            writeln!(self.out, "failures:").unwrap();
        }

        for (name, failures) in failed {
            for failure in failures {
                writeln!(
                    self.out,
                    "  {}:{}:0\t{}\t{}\n\t{}",
                    failure.file,
                    failure.line_nr,
                    name,
                    if let Some(index) = failure.param_index {
                        format!("{} ({})", failure.message, index)
                    } else {
                        failure.message.clone()
                    },
                    failure.details,
                )
                .unwrap();
            }
        }
    }

    fn skipped(&mut self, name: FqFnName<'static>, info: TestRunInfo, _reason: SkipReason) {
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
                make_test_name(name.name)
            )
            .unwrap();
        }
    }

    fn finished(
        &mut self,
        test: &'static TestCase,
        panic_message: Option<String>,
        elapsed: Duration,
    ) {
        let test_info = self.current_test_info.clone().unwrap();
        let duration = elapsed - self.started_at.unwrap();
        let test_updates = take(&mut self.test_updates);
        if test_info.run_count == 0 {
            writeln!(
                self.out,
                "{}|{: >8}|{}{} {}",
                if panic_message.is_some() { "!" } else { " " },
                format_time(duration),
                "  ".repeat(test_info.depth),
                if test_info.depth == 0 {
                    "Given".yellow()
                } else {
                    "When".yellow()
                },
                make_test_name(test.name.name)
            )
            .unwrap();
            if let Some(param) = &test_info.param_info {
                writeln!(
                    self.out,
                    " |        |{}{} {}",
                    "  ".repeat(test_info.depth),
                    "With".yellow(),
                    param
                        .headers
                        .iter()
                        .zip(param.display_str.iter())
                        .map(|(header, label)| format!("{}={}", header, label))
                        .collect::<Vec<String>>()
                        .join(","),
                )
                .unwrap();
            }

            let mut params_state = None;
            for (index, update) in test_updates.into_iter().enumerate() {
                match update {
                    TestUpdate::Verified {
                        message,
                        line_nr: _,
                        file: _,
                        outcome,
                    } => {
                        writeln!(
                            self.out,
                            "{}|       -|  {}{} {}",
                            get_assertion_status(&outcome),
                            "  ".repeat(test_info.depth),
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
                    }
                    TestUpdate::ParamsStarted {
                        message,
                        line_nr: _,
                        file: _,
                        header,
                    } => {
                        params_state = Some(ParamsState {
                            index,
                            message,
                            header,
                            outcomes: Default::default(),
                            rows_fields: Default::default(),
                        });
                    }
                    TestUpdate::ParamVerified {
                        index: _,
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
                            "  ".repeat(test_info.depth),
                            if state.index == 0 {
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
                            "  ".repeat(test_info.depth),
                            header.join(",")
                        )
                        .unwrap();

                        for (_index, (row, outcome)) in
                            state.rows_fields.iter().zip(state.outcomes).enumerate()
                        {
                            writeln!(
                                self.out,
                                "{}|       -|  {}|{} |",
                                get_assertion_status(&outcome),
                                "  ".repeat(test_info.depth),
                                if let VerifyOutcome::Success = &outcome {
                                    row.join(",")
                                } else {
                                    row.join(",").red().to_string()
                                },
                            )
                            .unwrap();
                        }
                    }
                };
            }
        }
    }
}

#[derive(Debug)]
struct BtSymbol<'a> {
    name: String,
    path: &'a Path,
    lineno: u32,
    colno: u32,
}

struct ParamsState {
    index: usize, // to determine whether it's Then or And
    message: String,
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

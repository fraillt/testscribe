use std::{io::Write, path::Path};

use backtrace::BacktraceFrame;

use crate::{logger::FailedTest, panic_hook::PanicDetails};

pub struct TestSummary {
    failed_tests: Vec<FailedTest>,
    panics: Vec<PanicDetails>,
}

impl TestSummary {
    pub fn new(failed_tests: Vec<FailedTest>, panics: Vec<PanicDetails>) -> Self {
        Self {
            failed_tests,
            panics,
        }
    }

    pub fn is_success(&self) -> bool {
        self.failed_tests.is_empty()
    }

    pub fn print_summary(&self, out: &mut dyn Write) {
        if !self.failed_tests.is_empty() {
            writeln!(out, "failures:").unwrap();
        }

        for test in &self.failed_tests {
            for failure in &test.failures {
                writeln!(
                    out,
                    "  {}:{}:0\t{}\t{}\n\t{}",
                    failure.file,
                    failure.line_nr,
                    test.name.name,
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

        if !self.panics.is_empty() {
            writeln!(out, "panic info:").unwrap();
        }

        for panic in &self.panics {
            writeln!(
                out,
                "loc | {}:{}:{}",
                panic.location.file, panic.location.line, panic.location.col
            )
            .unwrap();
            let msg = panic
                .message
                .clone()
                .map(|msg| msg.replace('\n', "\n    | "))
                .unwrap_or_else(|| "???".to_owned());
            writeln!(out, "msg | {msg}",).unwrap();
            let mut bt = panic.backtrace.clone();
            writeln!(out, "bt  |").unwrap();
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
                writeln!(out, "{:>3} | {}", index + 1, symbol.name).unwrap();
                writeln!(
                    out,
                    "    | {}:{}:{}",
                    symbol.path.to_string_lossy(),
                    symbol.lineno,
                    symbol.colno
                )
                .unwrap();
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

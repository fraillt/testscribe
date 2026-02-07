use std::panic::{PanicHookInfo, set_hook, take_hook};
use std::sync::{Mutex, MutexGuard, OnceLock};

use backtrace::Backtrace;
use serde::Serialize;

use testscribe_core::processor::panic::extract_string_from_panic_payload;

static PANIC_HOOK_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

#[derive(Debug, Clone, Serialize, PartialEq, Eq, Hash)]
pub struct PanicLocation {
    pub file: String,
    pub line: u32,
    pub col: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct PanicDetails {
    pub location: PanicLocation,
    pub backtrace: Backtrace,
    pub message: Option<String>,
}

pub struct PanicHandler {
    _panic_hook_lock: MutexGuard<'static, ()>,
}

impl Drop for PanicHandler {
    fn drop(&mut self) {
        let _ = take_hook();
    }
}

impl PanicHandler {
    pub fn attach_panic_hook(f: impl Fn(PanicDetails) + Send + Sync + 'static) -> Self {
        let _panic_hook_lock = PANIC_HOOK_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap();

        set_hook(Box::new(move |info| {
            f(create_panic_details(info, Backtrace::new_unresolved()))
        }));

        Self { _panic_hook_lock }
    }
}

fn create_panic_details(info: &PanicHookInfo, backtrace: Backtrace) -> PanicDetails {
    let location = match info.location() {
        Some(location) => PanicLocation {
            file: location.file().to_owned(),
            line: location.line(),
            col: location.column(),
        },
        None => PanicLocation {
            file: "???".to_owned(),
            line: 0,
            col: 0,
        },
    };
    PanicDetails {
        location,
        backtrace,
        message: extract_string_from_panic_payload(info.payload()),
    }
}

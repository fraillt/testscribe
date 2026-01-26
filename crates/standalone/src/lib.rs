// TODO this should be private, but it exposed for backend
pub mod logger;
pub mod panic_hook;
mod runner;

use futures::executor::block_on;
use runner::run_test;

use testscribe_core::test_case::TestCase;

use crate::runner::Config;

pub fn run_sync(
    test_cases: &'static [TestCase],
    module_path: &'static str,
    test_name: &'static str,
) -> Result<(), String> {
    let cfg = Config::from_args(std::env::args());
    block_on(run_test(test_cases, module_path, test_name, false, cfg))
        .map_err(|err| err.to_string())
}

pub async fn run_async(
    test_cases: &'static [TestCase],
    module_path: &'static str,
    test_name: &'static str,
) -> Result<(), String> {
    let cfg = Config::from_args(std::env::args());
    run_test(test_cases, module_path, test_name, true, cfg)
        .await
        .map_err(|err| err.to_string())
}

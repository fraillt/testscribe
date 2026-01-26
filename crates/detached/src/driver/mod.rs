pub mod cli;
pub mod remote;
use std::collections::BTreeMap;
use std::process::ExitCode;

use crate::runtime::interface::{CommandSender, Frontend, StatusReceiver};
use testscribe_core::test_case::FqFnName;
use testscribe_core::tests_tree::TestsTree;

pub enum FrontendWithFallback<T1, T2> {
    Main(T1),
    Fallback(T2),
}

impl<T1, T2> FrontendWithFallback<T1, T2> {
    pub fn new(main: Option<T1>, fallback: impl FnOnce() -> T2) -> Self {
        match main {
            Some(main) => Self::Main(main),
            None => Self::Fallback(fallback()),
        }
    }
}

impl<T1, T2> Frontend for FrontendWithFallback<T1, T2>
where
    T1: Frontend + Send,
    T2: Frontend + Send,
{
    async fn start(
        self,
        dags: BTreeMap<FqFnName<'static>, TestsTree>,
        command_sender: CommandSender,
        status_receiver: StatusReceiver,
    ) -> ExitCode {
        match self {
            FrontendWithFallback::Main(main) => {
                main.start(dags, command_sender, status_receiver).await
            }
            FrontendWithFallback::Fallback(fallback) => {
                fallback.start(dags, command_sender, status_receiver).await
            }
        }
    }
}

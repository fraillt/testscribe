use std::{collections::BTreeMap, future::Future, process::ExitCode};

use futures::channel::mpsc::{UnboundedReceiver, UnboundedSender};

use crate::runtime::messages::{CommandMsg, StatusMsg};
use testscribe_core::test_case::FqFnName;
use testscribe_core::tests_tree::TestsTree;

pub type CommandSender = UnboundedSender<CommandMsg>;
pub type StatusReceiver = UnboundedReceiver<StatusMsg>;

pub trait Frontend {
    fn start(
        self,
        dags: BTreeMap<FqFnName<'static>, TestsTree>,
        command_sender: CommandSender,
        status_receiver: StatusReceiver,
    ) -> impl Future<Output = ExitCode> + Send;
}

use std::time::Duration;

use futures::channel::mpsc::UnboundedSender;

use testscribe_core::{
    processor::logger::{Logger, TestStatusUpdate},
    test_case::TestCase,
};

pub struct TestStatusUpdateMsg {
    pub test: &'static TestCase,
    pub update: TestStatusUpdate,
    pub elapsed: Duration,
}

pub struct StatusSender<Msg> {
    tree_id: u64,
    logger: UnboundedSender<Msg>,
}

impl<Msg> StatusSender<Msg> {
    pub fn new(tree_id: u64, logger: UnboundedSender<Msg>) -> Self {
        Self { tree_id, logger }
    }
}

impl<Msg> Logger for StatusSender<Msg>
where
    Msg: From<(u64, TestStatusUpdateMsg)>,
{
    fn log(&mut self, test: &'static TestCase, update: TestStatusUpdate, elapsed: Duration) {
        self.logger
            .unbounded_send(
                (
                    self.tree_id,
                    TestStatusUpdateMsg {
                        test,
                        update,
                        elapsed,
                    },
                )
                    .into(),
            )
            .expect("Frontend must be alive")
    }
}

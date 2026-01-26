use std::time::{Duration, Instant};

use futures::channel::mpsc::UnboundedSender;

use testscribe_core::processor::logger::{Logger, TestStatusUpdate};

pub struct TestStatusUpdateMsg {
    pub update: TestStatusUpdate,
    pub elapsed: Duration,
}

pub struct StatusSender<Msg> {
    tree_id: u64,
    started_at: Instant,
    logger: UnboundedSender<Msg>,
}

impl<Msg> StatusSender<Msg> {
    pub fn new(tree_id: u64, started_at: Instant, logger: UnboundedSender<Msg>) -> Self {
        Self {
            tree_id,
            started_at,
            logger,
        }
    }
}

impl<Msg> Logger for StatusSender<Msg>
where
    Msg: From<(u64, TestStatusUpdateMsg)>,
{
    fn log(&mut self, update: TestStatusUpdate) {
        self.logger
            .unbounded_send(
                (
                    self.tree_id,
                    TestStatusUpdateMsg {
                        update,
                        elapsed: self.started_at.elapsed(),
                    },
                )
                    .into(),
            )
            .expect("Frontend must be alive")
    }
}

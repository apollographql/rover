mod message;
mod messenger;

pub(crate) use message::SubgraphMessage;
pub(crate) use messenger::SubgraphWatcherMessenger;

use crossbeam_channel::{bounded, Receiver, Sender};

#[derive(Debug, Clone)]
pub struct FollowerChannel {
    pub sender: Sender<SubgraphMessage>,
    pub receiver: Receiver<SubgraphMessage>,
}

impl FollowerChannel {
    pub fn new() -> Self {
        let (sender, receiver) = bounded(0);

        Self { sender, receiver }
    }
}

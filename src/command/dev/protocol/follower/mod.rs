mod message;
mod messenger;

pub(crate) use message::SubgraphUpdated;
pub(crate) use messenger::SubgraphWatcherMessenger;

// TODO: switch off of crossbeam
use crossbeam_channel::{bounded, Receiver, Sender};

#[derive(Debug, Clone)]
pub struct SubgraphMessageChannel {
    pub sender: Sender<SubgraphUpdated>,
    pub receiver: Receiver<SubgraphUpdated>,
}

impl SubgraphMessageChannel {
    pub fn new() -> Self {
        let (sender, receiver) = bounded(0);

        Self { sender, receiver }
    }
}

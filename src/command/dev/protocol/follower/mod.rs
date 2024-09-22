mod message;
mod messenger;

pub(crate) use message::SubgraphMessage;
pub(crate) use messenger::SubgraphWatcherMessenger;

// TODO: switch off of crossbeam
use crossbeam_channel::{bounded, Receiver, Sender};

#[derive(Debug, Clone)]
pub struct SubgraphMessageChannel {
    pub sender: Sender<SubgraphMessage>,
    pub receiver: Receiver<SubgraphMessage>,
}

impl SubgraphMessageChannel {
    pub fn new() -> Self {
        let (sender, receiver) = bounded(0);

        Self { sender, receiver }
    }
}

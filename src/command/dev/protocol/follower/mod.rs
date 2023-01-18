mod message;
mod messenger;

pub use message::*;
pub use messenger::*;

use crossbeam_channel::{bounded, Receiver, Sender};

#[derive(Debug, Clone)]
pub struct FollowerChannel {
    pub sender: Sender<FollowerMessage>,
    pub receiver: Receiver<FollowerMessage>,
}

impl FollowerChannel {
    pub fn new() -> Self {
        let (sender, receiver) = bounded(0);

        Self { sender, receiver }
    }
}

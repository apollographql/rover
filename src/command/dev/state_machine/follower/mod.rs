mod message;
mod messenger;

pub use message::*;
pub use messenger::*;

use tokio::sync::mpsc::{channel, Receiver, Sender};

#[derive(Debug, Clone)]
pub struct FollowerChannel {
    pub sender: Sender<FollowerMessage>,
    pub receiver: Receiver<FollowerMessage>,
}

impl FollowerChannel {
    pub fn new() -> Self {
        let (sender, receiver) = channel(1);

        Self { sender, receiver }
    }
}

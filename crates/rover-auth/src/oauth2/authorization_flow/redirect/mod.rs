use std::sync::Arc;

use tokio::sync::{Mutex, oneshot};

mod future;
mod handler;
/// Axum-based local server for handling the OAuth redirect callback.
pub mod server;

/// Default host for the local OAuth redirect server.
pub const DEFAULT_REDIRECT_HOST: &str = "127.0.0.1";

#[derive(Clone, Debug)]
struct OneshotSender<T> {
    sender: Arc<Mutex<Option<oneshot::Sender<T>>>>,
}

impl<T> OneshotSender<T> {
    pub async fn send(&mut self, message: T) -> Result<(), T> {
        let sender = self.sender.lock().await.take();
        match sender {
            Some(sender) => sender.send(message),
            None => Err(message),
        }
    }
}

impl<T> From<oneshot::Sender<T>> for OneshotSender<T> {
    fn from(value: oneshot::Sender<T>) -> Self {
        OneshotSender {
            sender: Arc::new(Mutex::new(Some(value))),
        }
    }
}

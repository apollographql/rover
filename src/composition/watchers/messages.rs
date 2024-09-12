use futures::{stream::BoxStream, StreamExt};
use tap::TapFallible;
use tokio::sync::mpsc::unbounded_channel;
use tokio_stream::wrappers::UnboundedReceiverStream;

use super::watcher::router_config::RouterConfigMessage;

pub fn receive_messages(
    mut router_config_messages: BoxStream<'static, RouterConfigMessage>,
) -> BoxStream<'static, RoverDevMessage> {
    let (tx, rx) = unbounded_channel();
    tokio::spawn(async move {
        while let Some(message) = router_config_messages.next().await {
            let message = RoverDevMessage::from(message);
            tx.send(message)
                .tap_err(|err| tracing::error!("{:?}", err))
                .unwrap();
        }
    });
    UnboundedReceiverStream::new(rx).boxed()
}

pub enum RoverDevMessage {
    Config(RouterConfigMessage),
}

impl From<RouterConfigMessage> for RoverDevMessage {
    fn from(value: RouterConfigMessage) -> Self {
        Self::Config(value)
    }
}

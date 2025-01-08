use futures::stream::BoxStream;
use futures::StreamExt;
use tap::TapFallible;
use tokio::sync::mpsc::UnboundedSender;
use tokio::task::AbortHandle;
use tracing::error;

use crate::composition::events::CompositionEvent;
use crate::composition::watchers::watcher::supergraph_config::{
    SupergraphConfigDiff, SupergraphConfigSerialisationError,
};
use crate::composition::CompositionError;
use crate::subtask::SubtaskHandleStream;

pub struct FederationWatcher {}

impl SubtaskHandleStream for FederationWatcher {
    type Input = Result<SupergraphConfigDiff, SupergraphConfigSerialisationError>;

    type Output = CompositionEvent;

    fn handle(
        self,
        sender: UnboundedSender<Self::Output>,
        mut input: BoxStream<'static, Self::Input>,
    ) -> AbortHandle {
        tokio::task::spawn(async move {
            while let Some(recv_res) = input.next().await {
                match recv_res {
                    Err(SupergraphConfigSerialisationError::DeserializingConfigError {
                        source,
                    }) => {
                        let _ = sender
                            .send(CompositionEvent::Error(
                                CompositionError::InvalidSupergraphConfig(source.message()),
                            ))
                            .tap_err(|err| error!("{:?}", err));
                    }
                    _ => (),
                }
            }
        })
        .abort_handle()
    }
}

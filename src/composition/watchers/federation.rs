use futures::stream::BoxStream;
use futures::StreamExt;
use tokio::sync::mpsc::UnboundedSender;
use tokio::task::AbortHandle;

use crate::composition::events::CompositionEvent;
use crate::composition::watchers::watcher::supergraph_config::{
    SupergraphConfigDiff, SupergraphConfigSerialisationError,
};
use crate::subtask::SubtaskHandleStream;

pub struct FederationWatcher {}

impl SubtaskHandleStream for FederationWatcher {
    type Input = Result<SupergraphConfigDiff, SupergraphConfigSerialisationError>;

    type Output = CompositionEvent;

    fn handle(
        self,
        _: UnboundedSender<Self::Output>,
        mut input: BoxStream<'static, Self::Input>,
    ) -> AbortHandle {
        tokio::task::spawn(async move {
            while let Some(recv_res) = input.next().await {
                match recv_res {
                    Err(SupergraphConfigSerialisationError::DeserializingConfigError {
                        ..
                    }) => {
                        tracing::error!("Here's your error!");
                    }
                    _ => (),
                }
            }
        })
        .abort_handle()
    }
}

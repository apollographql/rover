use futures::stream::BoxStream;
use futures::StreamExt;
use tap::TapFallible;
use tokio::sync::mpsc::UnboundedSender;
use tokio_util::sync::CancellationToken;
use tracing::error;

use crate::composition::events::CompositionEvent;
use crate::composition::watchers::composition::CompositionInputEvent;
use crate::composition::watchers::watcher::supergraph_config::{
    SupergraphConfigDiff, SupergraphConfigSerialisationError,
};
use crate::composition::CompositionError;
use crate::subtask::SubtaskHandleStream;

pub struct FederationWatcher {}

impl SubtaskHandleStream for FederationWatcher {
    type Input = Result<SupergraphConfigDiff, SupergraphConfigSerialisationError>;

    type Output = CompositionInputEvent;

    fn handle(
        self,
        sender: UnboundedSender<Self::Output>,
        mut input: BoxStream<'static, Self::Input>,
        cancellation_token: Option<CancellationToken>,
    ) {
        let cancellation_token = cancellation_token.unwrap_or_default();
        tokio::spawn(async move {
            let cancellation_token = cancellation_token.clone();
            cancellation_token
                .run_until_cancelled(async move {
                    while let Some(recv_res) = input.next().await {
                        match recv_res {
                            Ok(diff) => {
                                if let Some(fed_version) = diff.federation_version() {
                                    let _ = sender
                                        .send(CompositionInputEvent::Federation(
                                            fed_version.clone(),
                                        ))
                                        .tap_err(|err| error!("{:?}", err));
                                }
                            }
                            Err(SupergraphConfigSerialisationError::DeserializingConfigError {
                                source,
                            }) => {
                                let _ = sender
                                    .send(CompositionInputEvent::Passthrough(
                                        CompositionEvent::Error(
                                            CompositionError::InvalidSupergraphConfig(
                                                source.message(),
                                            ),
                                        ),
                                    ))
                                    .tap_err(|err| error!("{:?}", err));
                            }
                        }
                    }
                })
                .await;
        });
    }
}

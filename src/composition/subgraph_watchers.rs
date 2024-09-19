use std::collections::HashMap;

use apollo_federation_types::config::SupergraphConfig;
use tap::TapFallible;
use tokio::task::AbortHandle;
use tokio_stream::{wrappers::UnboundedReceiverStream, StreamExt};

use super::watchers::{
    subtask::{Subtask, SubtaskHandleStream, SubtaskRunUnit},
    watcher::{
        subgraph::{SubgraphChanged, SubgraphWatcher},
        supergraph_config::SupergraphConfigDiff,
    },
};

pub struct SubgraphWatchers {
    watchers: HashMap<
        String,
        (
            UnboundedReceiverStream<SubgraphChanged>,
            Subtask<SubgraphWatcher, SubgraphChanged>,
        ),
    >,
}

impl SubgraphWatchers {
    pub fn new(supergraph_config: SupergraphConfig) -> SubgraphWatchers {
        let watchers = supergraph_config
            .into_iter()
            .filter_map(|(name, subgraph_config)| {
                SubgraphWatcher::try_from((name.to_string(), subgraph_config.schema))
                    .tap_err(|err| tracing::warn!("Skipping subgraph {}: {:?}", name, err))
                    .ok()
                    .map(|value| (name, Subtask::new(value)))
            })
            .collect();
        SubgraphWatchers { watchers }
    }
}

impl SubtaskHandleStream for SubgraphWatchers {
    type Input = SupergraphConfigDiff;
    type Output = SubgraphChanged;
    fn handle(
        self,
        sender: tokio::sync::mpsc::UnboundedSender<Self::Output>,
        mut input: futures::stream::BoxStream<'static, Self::Input>,
    ) -> tokio::task::AbortHandle {
        tokio::task::spawn(async move {
            let mut abort_handles: HashMap<String, (AbortHandle, AbortHandle)> = HashMap::new();
            for (subgraph_name, (mut messages, subtask)) in self.watchers.into_iter() {
                let sender = sender.clone();
                let messages_abort_handle = tokio::task::spawn(async move {
                    while let Some(event) = messages.next().await {
                        let _ = sender
                            .send(event)
                            .tap_err(|err| tracing::error!("{:?}", err));
                    }
                })
                .abort_handle();
                let subtask_abort_handle = subtask.run();
                abort_handles.insert(subgraph_name, (messages_abort_handle, subtask_abort_handle));
            }

            // for supergraph diff events
            while let Some(diff) = input.next().await {
                // for new subgraphs added to the session
                for (name, subgraph_config) in diff.added() {
                    if let Ok((mut messages, subtask)) = SubgraphWatcher::try_from((
                        name.to_string(),
                        subgraph_config.schema.clone(),
                    ))
                    .map(Subtask::new)
                    .tap_err(|err| {
                        tracing::warn!("Cannot configure new subgraph for {name}: {:?}", err)
                    }) {
                        let sender = sender.clone();
                        let messages_abort_handle = tokio::spawn(async move {
                            while let Some(event) = messages.next().await {
                                let _ = sender
                                    .send(event)
                                    .tap_err(|err| tracing::error!("{:?}", err));
                            }
                        })
                        .abort_handle();
                        let subtask_abort_handle = subtask.run();
                        abort_handles.insert(
                            name.to_string(),
                            (messages_abort_handle, subtask_abort_handle),
                        );
                    }
                }
                for name in diff.removed() {
                    if let Some((messages_abort_handle, subtask_abort_handle)) =
                        abort_handles.get(name)
                    {
                        messages_abort_handle.abort();
                        subtask_abort_handle.abort();
                        abort_handles.remove(name);
                    }
                }
            }
        })
        .abort_handle()
    }
}

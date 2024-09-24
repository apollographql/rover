use std::{
    collections::HashMap,
    ops::DerefMut,
    sync::{Arc, OnceLock},
};

use apollo_federation_types::config::SupergraphConfig;
use rover_std::infoln;
use tap::TapFallible;
use tokio::sync::Mutex;
use tokio_stream::{wrappers::UnboundedReceiverStream, StreamExt};
use tokio_util::sync::CancellationToken;

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

    fn spawn_subgraph_subtask(
        sender: tokio::sync::mpsc::UnboundedSender<SubgraphChanged>,
        mut messages: UnboundedReceiverStream<SubgraphChanged>,
        subtask: Subtask<SubgraphWatcher, SubgraphChanged>,
    ) -> CancellationToken {
        let cancellation_token = CancellationToken::new();
        tokio::task::spawn({
            let cancellation_token = cancellation_token.clone();
            async move {
                let messages_abort_handle = Arc::new(OnceLock::new());
                let subtask_cancellation_token = Arc::new(OnceLock::new());
                tokio::select! {
                    _ = {
                        cancellation_token.cancelled()
                    } => {
                        if let Some(messages_abort_handle) = messages_abort_handle.get() {
                            messages_abort_handle.abort();
                        }
                        if let Some(subtask_cancellation_token) = subtask_cancellation_token.get() {
                            subtask_cancellation_token.cancel();
                        }
                    }
                    _ = {
                        let messages_abort_handle = messages_abort_handle.clone();
                        let subtask_cancellation_token = subtask_cancellation_token.clone();
                        async move {
                            let abort_handle = tokio::task::spawn(async move {
                                while let Some(event) = messages.next().await {
                                    let _ = sender
                                        .send(event)
                                        .tap_err(|err| tracing::error!("{:?}", err));
                                }
                            }).abort_handle();
                            let _ = messages_abort_handle.set(abort_handle).tap_err(|err| tracing::error!("{:?}", err));
                            let _ = subtask_cancellation_token.set(subtask.run()).tap_err(|err| tracing::error!("{:?}", err));
                        }
                    } => {}
                }
            }
        });
        cancellation_token
    }
}

impl SubtaskHandleStream for SubgraphWatchers {
    type Input = SupergraphConfigDiff;
    type Output = SubgraphChanged;
    fn handle(
        self,
        sender: tokio::sync::mpsc::UnboundedSender<Self::Output>,
        mut input: futures::stream::BoxStream<'static, Self::Input>,
    ) -> CancellationToken {
        let cancellation_token = CancellationToken::new();
        tokio::task::spawn({
            let cancellation_token = cancellation_token.clone();
            async move {
                let abort_handles: Arc<Mutex<HashMap<String, CancellationToken>>> =
                    Arc::new(Mutex::new(HashMap::new()));
                tokio::select! {
                    _ = cancellation_token.cancelled() => {
                        let abort_handles = abort_handles.clone();
                        let mut abort_handles = abort_handles.lock().await;
                        let abort_handles = abort_handles.deref_mut();
                        for (subgraph_name, abort_handle) in abort_handles.into_iter() {
                            infoln!("Shutting down subgraph: {}", subgraph_name);
                            abort_handle.cancel();
                        }
                        abort_handles.clear();
                    }
                    _ = {
                        let abort_handles = abort_handles.clone();
                        async move {

                            for (subgraph_name, (messages, subtask)) in self.watchers.into_iter() {
                                let sender = sender.clone();
                                let cancellation_token = Self::spawn_subgraph_subtask(sender, messages, subtask);
                                let mut abort_handles = abort_handles.lock().await;
                                let abort_handles = abort_handles.deref_mut();
                                abort_handles.insert(subgraph_name, cancellation_token);
                            }

                            // for supergraph diff events
                            while let Some(diff) = input.next().await {
                                // for new subgraphs added to the session
                                for (name, subgraph_config) in diff.added() {
                                    if let Ok((messages, subtask)) = SubgraphWatcher::try_from((
                                        name.to_string(),
                                        subgraph_config.schema.clone(),
                                    ))
                                        .map(Subtask::new)
                                        .tap_err(|err| {
                                            tracing::warn!("Cannot configure new subgraph for {name}: {:?}", err)
                                        }) {
                                            let sender = sender.clone();
                                            let cancellation_token = Self::spawn_subgraph_subtask(sender, messages, subtask);
                                            let mut abort_handles = abort_handles.lock().await;
                                            let abort_handles = abort_handles.deref_mut();
                                            abort_handles.insert(name.to_string(), cancellation_token);
                                        }
                                }
                                for name in diff.removed() {
                                    let mut abort_handles = abort_handles.lock().await;
                                    let abort_handles = abort_handles.deref_mut();
                                    if let Some(cancellation_token) = abort_handles.get(name)
                                    {
                                        cancellation_token.cancel();
                                        abort_handles.remove(name);
                                    }
                                }
                            }
                        }
                    } => {}
                }
            }
        });
        cancellation_token
    }
}

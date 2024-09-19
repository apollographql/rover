use std::collections::HashMap;

use apollo_federation_types::config::SupergraphConfig;
use futures::stream::{empty, StreamExt};
use tap::TapFallible;
use tokio::task::AbortHandle;
use tokio_stream::wrappers::UnboundedReceiverStream;

use crate::{
    composition::watchers::{
        subtask::{Subtask, SubtaskRunUnit},
        watcher::{
            file::FileWatcher, subgraph::SubgraphWatcher,
            supergraph_config::SupergraphConfigWatcher,
        },
    },
    utils::effect::{exec::TokioCommand, read_file::FsReadFile},
    RoverResult,
};

use super::{
    events::CompositionEvent,
    run_composition::RunComposition,
    supergraph::{binary::SupergraphBinary, config::FinalSupergraphConfig},
    watchers::{
        subtask::{SubtaskHandleStream, SubtaskRunStream},
        watcher::{subgraph::SubgraphChanged, supergraph_config::SupergraphConfigDiff},
    },
};

// TODO: handle retry flag for subgraphs (see rover dev help)
pub struct Runner {
    supergraph_config: FinalSupergraphConfig,
    supergraph_binary: SupergraphBinary,
}

impl Runner {
    pub fn new(
        supergraph_config: FinalSupergraphConfig,
        supergraph_binary: SupergraphBinary,
    ) -> Self {
        Self {
            supergraph_config,
            supergraph_binary,
        }
    }

    pub async fn run(self) -> RoverResult<UnboundedReceiverStream<CompositionEvent>> {
        let (supergraph_config_stream, supergraph_config_subtask) =
            match self.supergraph_config_subtask() {
                Some((supergraph_diff_stream, supergraph_config_subtask)) => (
                    supergraph_diff_stream.boxed(),
                    Some(supergraph_config_subtask),
                ),
                None => (empty().boxed(), None),
            };

        let subgraph_config_watchers = SubgraphWatchers::new(self.supergraph_config.clone().into());
        let (subgraph_changed_messages, subgraph_config_watchers_subtask) =
            Subtask::new(subgraph_config_watchers);

        let composition_handler = RunComposition::builder()
            .supergraph_config(self.supergraph_config)
            .supergraph_binary(self.supergraph_binary)
            .exec_command(TokioCommand::default())
            .read_file(FsReadFile::default())
            .build();
        let (composition_messages, composition_subtask) = Subtask::new(composition_handler);

        composition_subtask.run(subgraph_changed_messages.boxed());
        subgraph_config_watchers_subtask.run(supergraph_config_stream);
        if let Some(supergraph_config_subtask) = supergraph_config_subtask {
            supergraph_config_subtask.run();
        }

        Ok(composition_messages)
    }

    fn supergraph_config_subtask(
        &self,
    ) -> Option<(
        UnboundedReceiverStream<SupergraphConfigDiff>,
        Subtask<SupergraphConfigWatcher, SupergraphConfigDiff>,
    )> {
        let supergraph_config: SupergraphConfig = self.supergraph_config.clone().into();

        if let Some(origin_path) = self.supergraph_config.origin_path() {
            let f = FileWatcher::new(origin_path.clone());
            let watcher = SupergraphConfigWatcher::new(f, supergraph_config.clone());
            Some(Subtask::new(watcher))
        } else {
            None
        }
    }
}

struct SubgraphWatchers {
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
                SubgraphWatcher::try_from(subgraph_config.schema)
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
                    if let Ok((mut messages, subtask)) =
                        SubgraphWatcher::try_from(subgraph_config.schema.clone())
                            .map(Subtask::new)
                            .tap_err(|err| {
                                tracing::warn!(
                                    "Cannot configure new subgraph for {name}: {:?}",
                                    err
                                )
                            })
                    {
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

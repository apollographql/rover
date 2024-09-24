use std::{
    collections::{BTreeMap, HashSet},
    sync::{Arc, OnceLock},
};

use apollo_federation_types::config::{ConfigError, SubgraphConfig, SupergraphConfig};
use derive_getters::Getters;
use tap::TapFallible;
use tokio::sync::mpsc::UnboundedSender;
use tokio_stream::StreamExt;
use tokio_util::sync::CancellationToken;

use crate::composition::watchers::subtask::{Subtask, SubtaskHandleUnit, SubtaskRunUnit};

use super::file::FileWatcher;

pub struct SupergraphConfigWatcher {
    file_watcher: FileWatcher,
    supergraph_config: SupergraphConfig,
}

impl SupergraphConfigWatcher {
    pub fn new(
        file_watcher: FileWatcher,
        supergraph_config: SupergraphConfig,
    ) -> SupergraphConfigWatcher {
        SupergraphConfigWatcher {
            file_watcher,
            supergraph_config,
        }
    }
}

impl SubtaskHandleUnit for SupergraphConfigWatcher {
    type Output = SupergraphConfigDiff;
    fn handle(self, sender: UnboundedSender<Self::Output>) -> CancellationToken {
        let cancellation_token = CancellationToken::new();
        tokio::spawn({
            let cancellation_token = cancellation_token.clone();
            async move {
                let subtask_cancellation_token = Arc::new(OnceLock::new());
                tokio::select! {
                    _ = cancellation_token.cancelled() => {
                        let subtask_cancellation_token = subtask_cancellation_token.clone();
                        if let Some(subtask_cancellation_token) = subtask_cancellation_token.get() {
                            subtask_cancellation_token.cancel();
                        }
                    }
                    _ = {
                        let subtask_cancellation_token = subtask_cancellation_token.clone();
                        async move {
                            let mut latest_supergraph_config = self.supergraph_config.clone();
                            let (mut messages, subtask) = <Subtask<_, String>>::new(self.file_watcher.clone());
                            tokio::spawn(async move {
                                while let Some(contents) = messages.next().await {
                                    match SupergraphConfig::new_from_yaml(&contents) {
                                        Ok(supergraph_config) => {
                                            if let Ok(supergraph_config_diff) = SupergraphConfigDiff::new(
                                                &latest_supergraph_config,
                                                supergraph_config.clone(),
                                            ) {
                                                let _ = sender
                                                    .send(supergraph_config_diff)
                                                    .tap_err(|err| tracing::error!("{:?}", err));
                                            }
                                            latest_supergraph_config = supergraph_config;
                                        }
                                        Err(err) => {
                                            tracing::error!("Could not parse supergraph config file. {:?}", err);
                                            eprintln!("Could not parse supergraph config file");
                                        }
                                    }
                                }
                            });
                            let _ = subtask_cancellation_token.set(subtask.run()).tap_err(|err| tracing::error!("{:?}", err));
                        }
                    } => {}
                }
            }
        });
        cancellation_token
    }
}

#[derive(Getters)]
pub struct SupergraphConfigDiff {
    added: Vec<(String, SubgraphConfig)>,
    removed: Vec<String>,
}

impl SupergraphConfigDiff {
    pub fn new(
        old: &SupergraphConfig,
        new: SupergraphConfig,
    ) -> Result<SupergraphConfigDiff, ConfigError> {
        let old_subgraph_defs = old.get_subgraph_definitions().tap_err(|err| {
            eprintln!(
                "Error getting subgraph definitions from the current supergraph config: {:?}",
                err
            )
        })?;
        let new_subgraphs: BTreeMap<String, SubgraphConfig> = new.into_iter().collect();
        let old_subgraph_names: HashSet<String> =
            HashSet::from_iter(old_subgraph_defs.iter().map(|def| def.name.to_string()));
        let new_subgraph_names =
            HashSet::from_iter(new_subgraphs.keys().map(|name| name.to_string()));
        let added_names: HashSet<String> =
            HashSet::from_iter(new_subgraph_names.difference(&old_subgraph_names).cloned());
        let removed_names = old_subgraph_names.difference(&new_subgraph_names);
        let added = new_subgraphs
            .into_iter()
            .filter(|(name, _)| added_names.contains(name))
            .collect::<Vec<_>>();
        let removed = removed_names.into_iter().cloned().collect::<Vec<_>>();
        Ok(SupergraphConfigDiff { added, removed })
    }
}

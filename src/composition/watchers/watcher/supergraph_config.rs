use std::collections::{BTreeMap, HashSet};

use apollo_federation_types::config::{ConfigError, SubgraphConfig, SupergraphConfig};
use derive_getters::Getters;
use futures::StreamExt;
use tap::TapFallible;
use tokio::{sync::mpsc::UnboundedSender, task::AbortHandle};

use crate::composition::watchers::subtask::SubtaskHandleUnit;

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
    fn handle(self, sender: UnboundedSender<Self::Output>) -> AbortHandle {
        tokio::spawn(async move {
            let mut latest_supergraph_config = self.supergraph_config.clone();
            while let Some(contents) = self.file_watcher.clone().watch().next().await {
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
        })
        .abort_handle()
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

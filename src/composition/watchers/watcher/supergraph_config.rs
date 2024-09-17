use std::collections::HashSet;

use apollo_federation_types::{
    build::SubgraphDefinition,
    config::{ConfigError, SupergraphConfig},
};
use camino::Utf8PathBuf;
use derive_getters::Getters;
use futures::StreamExt;
use tap::TapFallible;

use super::file::FileWatcher;
use crate::composition::watchers::subtask::SubtaskHandleUnit;

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
    fn handle(
        self,
        sender: tokio::sync::mpsc::UnboundedSender<Self::Output>,
    ) -> tokio::task::AbortHandle {
        tokio::spawn(async move {
            let mut latest_supergraph_config = self.supergraph_config.clone();
            while let Some(contents) = self.file_watcher.clone().watch().next().await {
                match SupergraphConfig::new_from_yaml(&contents) {
                    Ok(supergraph_config) => {
                        if let Ok(supergraph_config_diff) = SupergraphConfigDiff::new(
                            &latest_supergraph_config,
                            &supergraph_config,
                            self.file_watcher.path.clone(),
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

// TODO: figure out if this is what we want or just a resolveduspergraphconfig
#[derive(Getters, Clone)]
pub struct SupergraphConfigDiff {
    added: Vec<SubgraphDefinition>,
    removed: Vec<String>,
    current: SupergraphConfig,
    path: Utf8PathBuf,
}

impl SupergraphConfigDiff {
    pub fn new(
        old: &SupergraphConfig,
        new: &SupergraphConfig,
        config_path: Utf8PathBuf,
    ) -> Result<SupergraphConfigDiff, ConfigError> {
        let old_subgraph_defs = old.get_subgraph_definitions().tap_err(|err| {
            eprintln!(
                "Error getting subgraph definitions from the current supergraph config: {:?}",
                err
            )
        })?;
        let old_subgraph_names: HashSet<String> =
            HashSet::from_iter(old_subgraph_defs.iter().map(|def| def.name.to_string()));
        let new_subgraph_defs = new.get_subgraph_definitions().tap_err(|err| {
            eprintln!(
                "Error getting subgraph definitions from the modified supergraph config: {:?}",
                err
            )
        })?;
        let new_subgraph_names =
            HashSet::from_iter(new_subgraph_defs.iter().map(|def| def.name.to_string()));
        let added_names: HashSet<String> =
            HashSet::from_iter(new_subgraph_names.difference(&old_subgraph_names).cloned());
        let removed_names = old_subgraph_names.difference(&new_subgraph_names);
        let added = new_subgraph_defs
            .into_iter()
            .filter(|def| added_names.contains(&def.name))
            .collect::<Vec<_>>();
        let removed = removed_names.into_iter().cloned().collect::<Vec<_>>();

        Ok(SupergraphConfigDiff {
            added,
            removed,
            // TODO: figure out how to handle this; we need the full config for composition, and
            // could either try to keep track of the added/removed or just send over the full sdl
            // (probs this and then the added/removed later?)
            current: new.clone(),
            path: config_path,
        })
    }
}

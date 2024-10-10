use std::collections::{BTreeMap, HashSet};

use apollo_federation_types::config::{ConfigError, SubgraphConfig, SupergraphConfig};
use derive_getters::Getters;
use futures::StreamExt;
use rover_std::errln;
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
                        tracing::error!("could not parse supergraph config file: {:?}", err);
                        errln!("could not parse supergraph config file: {:?}", err);
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
    changed: Vec<(String, SubgraphConfig)>,
    removed: Vec<String>,
}

impl SupergraphConfigDiff {
    /// Compares the differences between two supergraph configs,
    /// returning the added and removed subgraphs.
    pub fn new(
        old: &SupergraphConfig,
        new: SupergraphConfig,
    ) -> Result<SupergraphConfigDiff, ConfigError> {
        let old_subgraph_names: HashSet<String> = old
            .clone()
            .into_iter()
            .map(|(name, _config)| name)
            .collect();

        let new_subgraph_names: HashSet<String> = new
            .clone()
            .into_iter()
            .map(|(name, _config)| name)
            .collect();

        // Collect the subgraph definitions from the new supergraph config.
        let new_subgraphs: BTreeMap<String, SubgraphConfig> = new.clone().into_iter().collect();

        // Compare the old and new subgraph names to find additions.
        let added_names: HashSet<String> =
            HashSet::from_iter(new_subgraph_names.difference(&old_subgraph_names).cloned());

        // Compare the old and new subgraph names to find removals.
        let removed_names = old_subgraph_names.difference(&new_subgraph_names);

        // Filter the added and removed subgraphs from the new supergraph config.
        let added = new_subgraphs
            .into_iter()
            .filter(|(name, _)| added_names.contains(name))
            .collect::<Vec<_>>();
        let removed = removed_names.into_iter().cloned().collect::<Vec<_>>();

        // Find any in-place changes (eg, SDL, SchemaSource::Subgraph)
        let mut changed = vec![];
        for (old_name, old_config) in old.clone().into_iter() {
            // Exclude any removed subgraphs
            if !removed.contains(&old_name) {
                let found_new = new
                    .clone()
                    .into_iter()
                    .find(|(sub_name, _sub_config)| *sub_name == old_name);

                if let Some((old_name, new_config)) = found_new {
                    if new_config != old_config {
                        changed.push((old_name, new_config));
                    }
                }
            }
        }

        Ok(SupergraphConfigDiff {
            added,
            changed,
            removed,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use apollo_federation_types::config::{SchemaSource, SubgraphConfig, SupergraphConfig};

    use super::SupergraphConfigDiff;

    #[test]
    fn test_supergraph_config_diff() {
        // Construct a generic subgraph definition.
        let subgraph_def = SubgraphConfig {
            routing_url: Some("url".to_string()),
            schema: SchemaSource::Sdl {
                sdl: "sdl".to_string(),
            },
        };

        // Create an old supergraph config with subgraph definitions.
        let old_subgraph_defs: BTreeMap<String, SubgraphConfig> = BTreeMap::from([
            ("subgraph_a".to_string(), subgraph_def.clone()),
            ("subgraph_b".to_string(), subgraph_def.clone()),
        ]);
        let old = SupergraphConfig::new(old_subgraph_defs, None);

        // Create a new supergraph config with 1 new and 1 old subgraph definitions.
        let new_subgraph_defs: BTreeMap<String, SubgraphConfig> = BTreeMap::from([
            ("subgraph_a".to_string(), subgraph_def.clone()),
            ("subgraph_c".to_string(), subgraph_def.clone()),
        ]);
        let new = SupergraphConfig::new(new_subgraph_defs, None);

        // Assert diff contain correct additions and removals.
        let diff = SupergraphConfigDiff::new(&old, new).unwrap();
        assert_eq!(1, diff.added().len());
        assert_eq!(1, diff.removed().len());
        assert!(diff
            .added()
            .contains(&("subgraph_c".to_string(), subgraph_def.clone())));
        assert!(diff.removed().contains(&"subgraph_b".to_string()));
    }

    #[test]
    fn test_supergraph_config_diff_in_place_change() {
        let subgraph_def = SubgraphConfig {
            routing_url: None,
            schema: SchemaSource::Subgraph {
                graphref: "graph-ref".to_string(),
                subgraph: "subgraph".to_string(),
            },
        };

        let subgraph_def_updated = SubgraphConfig {
            routing_url: None,
            schema: SchemaSource::Subgraph {
                graphref: "updated-graph-ref".to_string(),
                subgraph: "subgraph".to_string(),
            },
        };

        // Create an old supergraph config with subgraph definitions.
        let old_subgraph_defs: BTreeMap<String, SubgraphConfig> =
            BTreeMap::from([("subgraph_a".to_string(), subgraph_def.clone())]);
        let old = SupergraphConfig::new(old_subgraph_defs, None);

        // Create a new supergraph config with 1 new and 1 old subgraph definitions.
        let new_subgraph_defs: BTreeMap<String, SubgraphConfig> =
            BTreeMap::from([("subgraph_a".to_string(), subgraph_def_updated.clone())]);
        let new = SupergraphConfig::new(new_subgraph_defs, None);

        // Assert diff contain correct additions and removals.
        let diff = SupergraphConfigDiff::new(&old, new).unwrap();

        assert_eq!(diff.changed().len(), 1);
        assert!(diff
            .changed()
            .contains(&("subgraph_a".to_string(), subgraph_def_updated.clone())));
    }
}

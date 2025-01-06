use std::collections::{BTreeMap, HashSet};

use apollo_federation_types::config::{ConfigError, SubgraphConfig, SupergraphConfig};
use derive_getters::Getters;
use futures::StreamExt;
use rover_std::errln;
use tap::TapFallible;
use tokio::{sync::mpsc::UnboundedSender, task::AbortHandle};

use crate::{
    composition::supergraph::config::{
        error::ResolveSubgraphError, lazy::LazilyResolvedSupergraphConfig,
        unresolved::UnresolvedSupergraphConfig,
    },
    subtask::SubtaskHandleUnit,
};

use super::file::FileWatcher;

#[derive(Debug)]
pub struct SupergraphConfigWatcher {
    file_watcher: FileWatcher,
    supergraph_config: SupergraphConfig,
}

impl SupergraphConfigWatcher {
    pub fn new(
        file_watcher: FileWatcher,
        supergraph_config: LazilyResolvedSupergraphConfig,
    ) -> SupergraphConfigWatcher {
        SupergraphConfigWatcher {
            file_watcher,
            supergraph_config: supergraph_config.into(),
        }
    }
}

impl SubtaskHandleUnit for SupergraphConfigWatcher {
    type Output = Result<SupergraphConfigDiff, BTreeMap<String, ResolveSubgraphError>>;

    fn handle(self, sender: UnboundedSender<Self::Output>) -> AbortHandle {
        tracing::warn!("Running SupergraphConfigWatcher");
        let supergraph_config_path = self.file_watcher.path().clone();
        tokio::spawn(
            async move {
                let supergraph_config_path = supergraph_config_path.clone();
                let mut latest_supergraph_config = self.supergraph_config.clone();
                let mut stream = self.file_watcher.watch().await;
                while let Some(contents) = stream.next().await {
                    eprintln!("{} changed. Applying changes to the session.", supergraph_config_path);
                    tracing::info!(
                        "{} changed. Parsing it as a `SupergraphConfig`",
                        supergraph_config_path
                    );
                    match SupergraphConfig::new_from_yaml(&contents) {
                        Ok(supergraph_config) => {
                            let subgraphs = BTreeMap::from_iter(supergraph_config.clone().into_iter());
                            let unresolved_supergraph_config = UnresolvedSupergraphConfig::builder()
                                .origin_path(supergraph_config_path.clone())
                                .subgraphs(subgraphs)
                                .build();
                            let supergraph_config = LazilyResolvedSupergraphConfig::resolve(
                                &supergraph_config_path.parent().unwrap().to_path_buf(),
                                unresolved_supergraph_config,
                            ).await.map(SupergraphConfig::from);

                            match supergraph_config {
                                Ok(supergraph_config) => {
                                    let supergraph_config_diff = SupergraphConfigDiff::new(
                                        &latest_supergraph_config,
                                        supergraph_config.clone(),
                                    );
                                    match supergraph_config_diff {
                                        Ok(supergraph_config_diff) =>  {
                                            let _ = sender
                                                .send(Ok(supergraph_config_diff))
                                                .tap_err(|err| tracing::error!("{:?}", err));
                                        }
                                        Err(err) => {
                                            tracing::error!("Failed to construct a diff between the current and previous `SupergraphConfig`s.\n{}", err);
                                        }
                                    }

                                    latest_supergraph_config = supergraph_config;
                                }
                                Err(err) => {
                                    errln!(
                                        "Failed to lazily resolve the supergraph config at {}.\n{}",
                                        supergraph_config_path,
                                        itertools::join(
                                            err
                                                .iter()
                                                .map(
                                                    |(name, err)| format!("{}: {}", name, err)
                                                ),
                                            "\n")
                                    );
                                    let _ = sender
                                        .send(Err(err))
                                        .tap_err(|err| tracing::error!("{:?}", err));
                                }
                            }
                        }
                        Err(err) => {
                            tracing::error!("could not parse supergraph config file: {:?}", err);
                            errln!("Could not parse supergraph config file.\n{}", err);
                        }
                    }
                }
        })
        .abort_handle()
    }
}

#[derive(Getters, Debug)]
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
            .clone()
            .into_iter()
            .filter(|(name, _)| added_names.contains(name))
            .collect::<Vec<_>>();
        let removed = removed_names.into_iter().cloned().collect::<Vec<_>>();

        // Find any in-place changes (eg, SDL, SchemaSource::Subgraph)
        let changed = old
            .clone()
            .into_iter()
            .filter(|(old_name, _)| !removed.contains(old_name))
            .filter_map(|(old_name, old_subgraph)| {
                new_subgraphs.get(&old_name).and_then(|new_subgraph| {
                    let new_subgraph = new_subgraph.clone();
                    if old_subgraph == new_subgraph {
                        None
                    } else {
                        Some((old_name, new_subgraph))
                    }
                })
            })
            .collect::<Vec<_>>();

        Ok(SupergraphConfigDiff {
            added,
            changed,
            removed,
        })
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
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

    #[rstest]
    #[case::schemasource_subgraph(
        SubgraphConfig {
            routing_url: None,
            schema: SchemaSource::Subgraph {
                graphref: "graph-ref".to_string(),
                subgraph: "subgraph".to_string(),
            },
        },
        SubgraphConfig {
            routing_url: None,
            schema: SchemaSource::Subgraph {
                graphref: "updated-graph-ref".to_string(),
                subgraph: "subgraph".to_string(),
            },
        }
    )]
    #[case::schemasource_sdl(
        SubgraphConfig {
            routing_url: None,
            schema: SchemaSource::Sdl { sdl: "old sdl".to_string() }
        },
        SubgraphConfig {
            routing_url: None,
            schema: SchemaSource::Sdl { sdl: "new sdl".to_string() }
        }
    )]
    fn test_supergraph_config_diff_in_place_change(
        #[case] old_subgraph_config: SubgraphConfig,
        #[case] new_subgraph_config: SubgraphConfig,
    ) {
        // Create an old supergraph config with subgraph definitions.
        let old_subgraph_defs: BTreeMap<String, SubgraphConfig> =
            BTreeMap::from([("subgraph_a".to_string(), old_subgraph_config.clone())]);
        let old = SupergraphConfig::new(old_subgraph_defs, None);

        // Create a new supergraph config with 1 new and 1 old subgraph definitions.
        let new_subgraph_defs: BTreeMap<String, SubgraphConfig> =
            BTreeMap::from([("subgraph_a".to_string(), new_subgraph_config.clone())]);
        let new = SupergraphConfig::new(new_subgraph_defs, None);

        // Assert diff contain correct additions and removals.
        let diff = SupergraphConfigDiff::new(&old, new).unwrap();

        assert_eq!(diff.changed().len(), 1);
        assert!(diff
            .changed()
            .contains(&("subgraph_a".to_string(), new_subgraph_config.clone())));
    }
}

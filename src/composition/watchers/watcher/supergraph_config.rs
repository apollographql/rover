use std::collections::{BTreeMap, HashSet};
use std::fmt::{Display, Formatter};
use std::sync::Arc;

use apollo_federation_types::config::{
    ConfigError, FederationVersion, SubgraphConfig, SupergraphConfig,
};
use derive_getters::Getters;
use futures::StreamExt;
use rover_std::errln;
use tap::TapFallible;
use thiserror::Error;
use tokio::sync::broadcast::Sender;
use tokio_util::sync::CancellationToken;
use tracing::debug;

use super::file::FileWatcher;
use crate::composition::supergraph::config::federation::FederationVersionResolver;
use crate::composition::supergraph::config::{
    error::ResolveSubgraphError, lazy::LazilyResolvedSupergraphConfig,
    unresolved::UnresolvedSupergraphConfig,
};
use crate::composition::watchers::watcher::supergraph_config::SupergraphConfigSerialisationError::DeserializingConfigError;
use crate::subtask::SubtaskHandleMultiStream;

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

impl SubtaskHandleMultiStream for SupergraphConfigWatcher {
    type Output = Result<SupergraphConfigDiff, SupergraphConfigSerialisationError>;

    fn handle(self, sender: Sender<Self::Output>, cancellation_token: Option<CancellationToken>) {
        let supergraph_config_path = self.file_watcher.path().clone();
        let cancellation_token = cancellation_token.unwrap_or_default();
        tokio::spawn(async move {
            let supergraph_config_path = supergraph_config_path.clone();
            let mut latest_supergraph_config = self.supergraph_config.clone();
            let mut broken = false;
            // Look at the current contents of the supergraph_config and emit an event if there's
            // a problem parsing it, otherwise move into the watching loop.
            if let Ok(contents) = self.file_watcher.fetch().await {
                if let Err(e) = SupergraphConfig::new_from_yaml(&contents) {
                    broken = true;
                    tracing::error!("could not parse supergraph config file: {:?}", e);
                    errln!("Could not parse supergraph config file.\n{}", e);
                    let _ = sender
                        .send(Err(DeserializingConfigError {
                            source: Arc::new(e),
                        }))
                        .tap_err(|err| tracing::error!("{:?}", err));
                }
            }

            let mut stream = self.file_watcher.watch().await;
            cancellation_token.run_until_cancelled(async move {
                    while let Some(contents) = stream.next().await {
                        eprintln!("{} changed. Applying changes to the session.", supergraph_config_path);
                        tracing::info!(
                                "{} changed. Parsing it as a `SupergraphConfig`",
                                supergraph_config_path
                            );
                        debug!("Current supergraph config is: {:?}", latest_supergraph_config);
                        match SupergraphConfig::new_from_yaml(&contents) {
                            Ok(mut supergraph_config) => {
                                let subgraphs = BTreeMap::from_iter(supergraph_config.clone().into_iter());
                                let unresolved_supergraph_config = UnresolvedSupergraphConfig::builder()
                                    .origin_path(supergraph_config_path.clone())
                                    .subgraphs(subgraphs)
                                    .federation_version_resolver(FederationVersionResolver::default().from_supergraph_config(Some(&supergraph_config)))
                                    .build();
                                let (lazily_resolved_supergraph_config, errors) = LazilyResolvedSupergraphConfig::resolve(
                                    &supergraph_config_path.parent().unwrap().to_path_buf(),
                                    unresolved_supergraph_config,
                                ).await;

                                let supergraph_config_diff = SupergraphConfigDiff::new(
                                    &latest_supergraph_config,
                                    SupergraphConfig::from(lazily_resolved_supergraph_config),
                                    errors.clone(),
                                    broken
                                );
                                match supergraph_config_diff {
                                    Ok(supergraph_config_diff) => {
                                        debug!("{supergraph_config_diff}");
                                        let _ = sender
                                            .send(Ok(supergraph_config_diff))
                                            .tap_err(|err| tracing::error!("{:?}", err));
                                    }
                                    Err(err) => {
                                        tracing::error!("Failed to construct a diff between the current and previous `SupergraphConfig`s.\n{}", err);
                                    }
                                }

                                supergraph_config = supergraph_config.into_iter().filter(|(name,_)| !errors.contains_key(name)).collect();

                                latest_supergraph_config = supergraph_config;
                                broken = false;
                            }
                            Err(err) => {
                                broken = true;
                                let old_fed_version = latest_supergraph_config.get_federation_version().clone();
                                latest_supergraph_config = SupergraphConfig::new(BTreeMap::new(),old_fed_version);
                                tracing::error!("could not parse supergraph config file: {:?}", err);
                                errln!("Could not parse supergraph config file.\n{}", err);
                                let _ = sender
                                    .send(Err(DeserializingConfigError {
                                        source: Arc::new(err)
                                    }))
                                    .tap_err(|err| tracing::error!("{:?}", err));
                            }
                        }
                    }
                }).await;
        });
    }
}

#[derive(Getters, Debug, Clone)]
pub struct SupergraphConfigDiff {
    added: Vec<(String, SubgraphConfig)>,
    changed: Vec<(String, SubgraphConfig)>,
    removed: Vec<(String, Option<ResolveSubgraphError>)>,
    federation_version: Option<Option<FederationVersion>>,
    previously_broken: bool,
}

impl Display for SupergraphConfigDiff {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Added: {:?} Changed: {:?} Removed: {:?} Previously Broken? {}",
            self.added.iter().map(|(name, _)| name).collect::<Vec<_>>(),
            self.changed
                .iter()
                .map(|(name, _)| name)
                .collect::<Vec<_>>(),
            self.removed,
            self.previously_broken
        )
    }
}

impl SupergraphConfigDiff {
    /// Compares the differences between two supergraph configs,
    /// returning the added and removed subgraphs.
    pub fn new(
        old: &SupergraphConfig,
        new: SupergraphConfig,
        resolution_errors: BTreeMap<String, ResolveSubgraphError>,
        previously_broken: bool,
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

        let federation_version = if old.get_federation_version() != new.get_federation_version() {
            debug!(
                "Detected federation version change. Changing from {:?} to {:?}",
                old.get_federation_version(),
                new.get_federation_version()
            );
            Some(new.get_federation_version())
        } else {
            None
        };

        let enriched_removed = removed
            .iter()
            .map(|name| {
                let potential_error = resolution_errors.get(name).cloned();
                (name.clone(), potential_error)
            })
            .collect();

        Ok(SupergraphConfigDiff {
            added,
            changed,
            removed: enriched_removed,
            federation_version,
            previously_broken,
        })
    }

    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.changed.is_empty() && self.removed.is_empty()
    }
}

#[derive(Error, Clone, Debug)]
pub enum SupergraphConfigSerialisationError {
    #[error("Variant which denotes errors came from trying to deserialise the Supergraph Config via apollo-federation-types")]
    DeserializingConfigError { source: Arc<ConfigError> },
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use apollo_federation_types::config::{SchemaSource, SubgraphConfig, SupergraphConfig};
    use rstest::rstest;

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
        let diff = SupergraphConfigDiff::new(&old, new, BTreeMap::default(), false).unwrap();
        assert_eq!(1, diff.added().len());
        assert_eq!(1, diff.removed().len());
        assert!(diff
            .added()
            .contains(&("subgraph_c".to_string(), subgraph_def.clone())));
        assert!(diff.removed().iter().any(|(name, _)| name == "subgraph_b"));
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
        let diff = SupergraphConfigDiff::new(&old, new, BTreeMap::default(), false).unwrap();

        assert_eq!(diff.changed().len(), 1);
        assert!(diff
            .changed()
            .contains(&("subgraph_a".to_string(), new_subgraph_config.clone())));
    }
}

use std::collections::{BTreeMap, HashSet};
use std::fmt::{Display, Formatter};
use std::sync::Arc;

use apollo_federation_types::config::{
    ConfigError, ConfigResult, FederationVersion, SubgraphConfig,
};
use camino::Utf8PathBuf;
use derive_getters::Getters;
use futures::StreamExt;
use rover_std::errln;
use tap::TapFallible;
use thiserror::Error;
use tokio::sync::broadcast::Sender;
use tokio_util::sync::CancellationToken;
use tracing::debug;

use super::file::FileWatcher;
use crate::composition::supergraph::config::error::ResolveSubgraphError;
use crate::composition::supergraph::config::federation::FederationVersionResolver;
use crate::composition::supergraph::config::full::introspect::ResolveIntrospectSubgraphFactory;
use crate::composition::supergraph::config::full::FullyResolvedSupergraphConfig;
use crate::composition::supergraph::config::lazy::LazilyResolvedSupergraphConfig;
use crate::composition::supergraph::config::resolver::fetch_remote_subgraph::FetchRemoteSubgraphFactory;
use crate::composition::supergraph::config::unresolved::UnresolvedSupergraphConfig;
use crate::composition::supergraph::config::SupergraphConfigYaml;
use crate::composition::watchers::watcher::supergraph_config::SupergraphConfigSerialisationError::DeserializingConfigError;
use crate::utils::expansion::expand;

/// Watches a `supergraph.yaml` file and emits [`SupergraphConfigDiff`]s
#[derive(Debug)]
pub(crate) struct SupergraphConfigWatcher {
    file_watcher: FileWatcher,
    supergraph_config: SupergraphConfigYaml,
    fetch_remote_subgraph_factory: FetchRemoteSubgraphFactory,
    resolve_introspect_subgraph_factory: ResolveIntrospectSubgraphFactory,
}

impl SupergraphConfigWatcher {
    pub fn new(
        file_watcher: FileWatcher,
        supergraph_config: LazilyResolvedSupergraphConfig,
        fetch_remote_subgraph_factory: FetchRemoteSubgraphFactory,
        resolve_introspect_subgraph_factory: ResolveIntrospectSubgraphFactory,
    ) -> SupergraphConfigWatcher {
        SupergraphConfigWatcher {
            file_watcher,
            supergraph_config: supergraph_config.into(),
            fetch_remote_subgraph_factory,
            resolve_introspect_subgraph_factory,
        }
    }

    /// Method that generates the set of LazilyResolvedSubgraphs(s), that can be successfully
    /// fully resolved at a later date. Because we want to know if a LazilyResolvedSupergraphConfig
    /// is valid we have to try and resolve it completely, however ultimately we want to get
    /// a set of LazilyResolvedSubgraphs out of the otherside otherwise comparing them is
    /// going to be impossible, and we'll miss many changes.
    ///
    /// As such this method, generates both versions, then filters the lazily resolved versions
    /// and returns that along with any errors from the full resolution process.
    async fn generate_correct_lazily_resolved_supergraph_config(
        supergraph_config_path: &Utf8PathBuf,
        mut unresolved_supergraph_config: UnresolvedSupergraphConfig,
        errors: BTreeMap<String, ResolveSubgraphError>,
    ) -> SupergraphConfigYaml {
        // First filter out the subgraphs from the unresolved set
        unresolved_supergraph_config.filter_subgraphs(errors.keys().cloned().collect());
        // Then resolve the filtered version, rather than the whole thing
        let (lazily_resolved_supergraph_config, _) = LazilyResolvedSupergraphConfig::resolve(
            &supergraph_config_path.parent().unwrap().to_path_buf(),
            unresolved_supergraph_config,
        )
        .await;
        debug!(
            "Filtered Lazily Resolved Supergraph Config: {:?}",
            lazily_resolved_supergraph_config
        );
        lazily_resolved_supergraph_config.into()
    }

    /// Spawn the watcher, sending diffs to the provided `sender`.
    pub(crate) fn run(
        self,
        sender: Sender<Result<SupergraphConfigDiff, SupergraphConfigSerialisationError>>,
    ) {
        let supergraph_config_path = self.file_watcher.path().clone();
        let cancellation_token = CancellationToken::new();
        tokio::spawn(async move {
            let supergraph_config_path = supergraph_config_path.clone();
            let mut latest_supergraph_config = self.supergraph_config.clone();
            let mut broken = false;
            // Look at the current contents of the supergraph_config and emit an event if there's
            // a problem parsing it, otherwise move into the watching loop.
            if let Ok(contents) = self.file_watcher.fetch().await {
                if let Err(e) = Self::read_supergraph_config(&contents) {
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

            let mut stream = self
                .file_watcher
                .clone()
                .watch(cancellation_token.clone())
                .await;
            cancellation_token.run_until_cancelled(async move {
                while let Some(contents) = stream.next().await {
                    eprintln!("{supergraph_config_path} changed. Applying changes to the session.");
                    tracing::info!(
                            "{} changed. Parsing it as a `SupergraphConfig`",
                            supergraph_config_path
                        );
                    debug!("Current supergraph config is: {:?}", latest_supergraph_config);
                    match Self::read_supergraph_config(&contents) {
                        Ok(supergraph_config) => {
                            let unresolved_supergraph_config = UnresolvedSupergraphConfig {
                                origin_path: Some(supergraph_config_path.clone()),
                                federation_version_resolver: Some(FederationVersionResolver::default().from_supergraph_config(Some( &supergraph_config))),
                                subgraphs: supergraph_config.subgraphs,
                            };
                            // Here we can throw away what actually gets resolved because we care about the fact it
                            // happens not the resulting artifact.
                            let errors = if let Ok((_, errors)) = FullyResolvedSupergraphConfig::resolve(
                                self.resolve_introspect_subgraph_factory.clone(),
                                self.fetch_remote_subgraph_factory.clone(),
                                &supergraph_config_path.parent().unwrap().to_path_buf(),
                                unresolved_supergraph_config.clone(),
                            ).await {
                                errors
                            } else {
                                tracing::error!("Could not fully resolve SupergraphConfig, will retry on next file change");
                                continue
                            };
                            let new_supergraph_config = Self::generate_correct_lazily_resolved_supergraph_config(&supergraph_config_path, unresolved_supergraph_config, errors.clone()).await;

                            let supergraph_config_diff = SupergraphConfigDiff::new(
                                latest_supergraph_config,
                                new_supergraph_config.clone(),
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

                            latest_supergraph_config = new_supergraph_config;
                            broken = false;
                        }
                        Err(err) => {
                            broken = true;
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

    /// Read the supergraph config from YAML contents and expand any variables
    fn read_supergraph_config(contents: &str) -> ConfigResult<SupergraphConfigYaml> {
        fn to_config_err(e: impl ToString) -> ConfigError {
            ConfigError::InvalidConfiguration {
                message: e.to_string(),
            }
        }
        let yaml_contents = expand(serde_yaml::from_str(contents).map_err(to_config_err)?)
            .map_err(to_config_err)?;
        serde_yaml::from_value(yaml_contents).map_err(to_config_err)
    }
}

#[derive(Getters, Debug, Clone)]
pub struct SupergraphConfigDiff {
    added: Vec<(String, SubgraphConfig)>,
    changed: Vec<(String, SubgraphConfig)>,
    removed: Vec<(String, Option<ResolveSubgraphError>)>,
    federation_version: Option<FederationVersion>,
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
        old: SupergraphConfigYaml,
        new: SupergraphConfigYaml,
        resolution_errors: BTreeMap<String, ResolveSubgraphError>,
        previously_broken: bool,
    ) -> Result<SupergraphConfigDiff, ConfigError> {
        debug!("Old Supergraph Config: {:?}", old);
        debug!("New Supergraph Config: {:?}", new);

        let old_subgraph_names: HashSet<String> = old.subgraphs.keys().cloned().collect();
        let new_subgraph_names: HashSet<String> = new.subgraphs.keys().cloned().collect();

        // Collect the subgraph definitions from the new supergraph config.
        let new_subgraphs: BTreeMap<String, SubgraphConfig> = new.subgraphs;
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
            .subgraphs
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

        let federation_version = if old.federation_version != new.federation_version {
            debug!(
                "Detected federation version change. Changing from {:?} to {:?}",
                old.federation_version, new.federation_version,
            );
            new.federation_version
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
    use super::*;
    use std::collections::BTreeMap;

    use apollo_federation_types::config::{ConfigError, SchemaSource, SubgraphConfig};
    use rstest::rstest;

    use crate::composition::watchers::watcher::supergraph_config::SupergraphConfigWatcher;

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
        let old = SupergraphConfigYaml {
            subgraphs: old_subgraph_defs,
            ..Default::default()
        };

        // Create a new supergraph config with 1 new and 1 old subgraph definitions.
        let new_subgraph_defs: BTreeMap<String, SubgraphConfig> = BTreeMap::from([
            ("subgraph_a".to_string(), subgraph_def.clone()),
            ("subgraph_c".to_string(), subgraph_def.clone()),
        ]);
        let new = SupergraphConfigYaml {
            subgraphs: new_subgraph_defs,
            ..Default::default()
        };

        // Assert diff contain correct additions and removals.
        let diff = SupergraphConfigDiff::new(old, new, BTreeMap::default(), false).unwrap();
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
        let old = SupergraphConfigYaml {
            subgraphs: old_subgraph_defs,
            ..Default::default()
        };

        // Create a new supergraph config with 1 new and 1 old subgraph definitions.
        let new_subgraph_defs: BTreeMap<String, SubgraphConfig> =
            BTreeMap::from([("subgraph_a".to_string(), new_subgraph_config.clone())]);
        let new = SupergraphConfigYaml {
            subgraphs: new_subgraph_defs,
            ..Default::default()
        };

        // Assert diff contain correct additions and removals.
        let diff = SupergraphConfigDiff::new(old, new, BTreeMap::default(), false).unwrap();

        assert_eq!(diff.changed().len(), 1);
        assert!(diff
            .changed()
            .contains(&("subgraph_a".to_string(), new_subgraph_config.clone())));
    }

    #[tokio::test]
    async fn test_environment_variable_expansion() {
        let yaml_config = r#"
            subgraphs:
              test_subgraph:
                routing_url: "http://localhost:${env.TEST_SUBGRAPH_PORT}/graphql"
                schema:
                  sdl: |
                    type Query {
                      hello: String
                    }
            "#;
        std::env::set_var("TEST_SUBGRAPH_PORT", "4000");
        let routing_url = SupergraphConfigWatcher::read_supergraph_config(yaml_config)
            .unwrap()
            .subgraphs
            .into_iter()
            .find(|(name, _)| name == "test_subgraph")
            .map(|(_, config)| config)
            .unwrap()
            .routing_url;
        assert_eq!(
            routing_url,
            Some(String::from("http://localhost:4000/graphql"))
        );
        std::env::remove_var("TEST_SUBGRAPH_PORT");
    }

    #[tokio::test]
    async fn test_environment_variable_expansion_not_defined() {
        let yaml_config = r#"
            subgraphs:
              test_subgraph:
                routing_url: "http://localhost:${env.NOT_DEFINED}/graphql"
                schema:
                  sdl: |
                    type Query {
                      hello: String
                    }
            "#;
        match SupergraphConfigWatcher::read_supergraph_config(yaml_config) {
            Err(ConfigError::InvalidConfiguration { message }) => {
                assert!(message.contains("environment variable not found"))
            }
            _ => panic!("Expected error"),
        }
    }
}

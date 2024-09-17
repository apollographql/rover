use apollo_federation_types::config::{ConfigError, SupergraphConfig};
use camino::Utf8PathBuf;
use derive_getters::Getters;
use rover_client::shared::GraphRef;
use rover_std::{Fs, RoverStdError};

use crate::{
    utils::{
        effect::{fetch_remote_subgraph::FetchRemoteSubgraph, introspect::IntrospectSubgraph},
        parsers::FileDescriptorType,
    },
    RoverError,
};

use self::{
    remote_subgraphs::FetchRemoteSubgraphs,
    resolve::{
        subgraph::{FullyResolvedSubgraph, LazilyResolvedSubgraph, ResolveSubgraphError},
        ResolvedSupergraphConfig, UnresolvedSupergraphConfig,
    },
    state::{LoadRemoteSubgraphs, ResolveSubgraphs, Writing},
};

mod remote_subgraphs;
pub mod resolve;

mod state {
    use apollo_federation_types::config::SupergraphConfig;

    pub struct LoadSupergraphConfig;
    pub struct LoadRemoteSubgraphs {
        pub supergraph_config: Option<SupergraphConfig>,
    }
    pub struct ResolveSubgraphs {
        pub supergraph_config: Option<SupergraphConfig>,
    }
    pub struct Writing {
        pub supergraph_config: SupergraphConfig,
    }
}

use state::LoadSupergraphConfig;

pub struct IntermediateSupergraphConfig<State> {
    state: State,
}

impl<T> IntermediateSupergraphConfig<T> {
    pub fn new() -> IntermediateSupergraphConfig<LoadSupergraphConfig> {
        IntermediateSupergraphConfig {
            state: LoadSupergraphConfig,
        }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum LoadSupergraphConfigError {
    #[error("Failed to parse the supergraph config")]
    SupergraphConfig(ConfigError),
    #[error("Failed to read file descriptor")]
    ReadFileDescriptor(RoverError),
}

impl IntermediateSupergraphConfig<LoadSupergraphConfig> {
    pub fn load_from_file_descriptor(
        self,
        file_descriptor_type: Option<FileDescriptorType>,
    ) -> Result<IntermediateSupergraphConfig<LoadRemoteSubgraphs>, LoadSupergraphConfigError> {
        if let Some(file_descriptor_type) = file_descriptor_type {
            let supergraph_config = file_descriptor_type
                .read_file_descriptor("supergraph config", &mut std::io::stdin())
                .map_err(LoadSupergraphConfigError::ReadFileDescriptor)
                .and_then(|contents| {
                    SupergraphConfig::new_from_yaml(&contents)
                        .map_err(LoadSupergraphConfigError::SupergraphConfig)
                })?;
            Ok(IntermediateSupergraphConfig {
                state: LoadRemoteSubgraphs {
                    supergraph_config: Some(supergraph_config),
                },
            })
        } else {
            Ok(IntermediateSupergraphConfig {
                state: LoadRemoteSubgraphs {
                    supergraph_config: None,
                },
            })
        }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum LoadRemoteSubgraphsError {
    #[error(transparent)]
    FetchRemoteSubgraphsError(Box<dyn std::error::Error>),
}

impl IntermediateSupergraphConfig<LoadRemoteSubgraphs> {
    pub async fn load_remote_subgraphs(
        self,
        fetch_remote_subgraphs_impl: &impl FetchRemoteSubgraphs,
        graph_ref: Option<&GraphRef>,
    ) -> Result<IntermediateSupergraphConfig<ResolveSubgraphs>, LoadRemoteSubgraphsError> {
        if let Some(graph_ref) = graph_ref {
            let remote_supergraph_config = fetch_remote_subgraphs_impl
                .fetch_remote_subgraphs(graph_ref)
                .await
                .map_err(|err| {
                    LoadRemoteSubgraphsError::FetchRemoteSubgraphsError(Box::new(err))
                })?;
            Ok(IntermediateSupergraphConfig {
                state: ResolveSubgraphs {
                    supergraph_config: self
                        .state
                        .supergraph_config
                        .map(|mut supergraph_config| {
                            supergraph_config.merge_subgraphs(&remote_supergraph_config);
                            supergraph_config
                        })
                        .or_else(|| Some(remote_supergraph_config)),
                },
            })
        } else {
            Ok(IntermediateSupergraphConfig {
                state: ResolveSubgraphs {
                    supergraph_config: self.state.supergraph_config,
                },
            })
        }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum ResolveSupergraphConfigError {
    #[error("No source found for supergraph config")]
    NoSource,
    #[error("Unable to resolve subgraphs")]
    ResolveSubgraphs(Vec<ResolveSubgraphError>),
}

impl IntermediateSupergraphConfig<ResolveSubgraphs> {
    pub async fn fully_resolve_subgraphs<CTX>(
        self,
        introspect_subgraph_impl: &impl IntrospectSubgraph,
        fetch_remote_subgraph_impl: &impl FetchRemoteSubgraph,
        supergraph_config_root: &Utf8PathBuf,
    ) -> Result<IntermediateSupergraphConfig<Writing>, ResolveSupergraphConfigError>
    where
        CTX: IntrospectSubgraph + FetchRemoteSubgraph,
    {
        match self.state.supergraph_config {
            Some(supergraph_config) => {
                let unresolved_supergraph_config =
                    UnresolvedSupergraphConfig::new(supergraph_config);
                let resolved_supergraph_config =
                    <ResolvedSupergraphConfig<FullyResolvedSubgraph>>::resolve(
                        introspect_subgraph_impl,
                        fetch_remote_subgraph_impl,
                        supergraph_config_root,
                        unresolved_supergraph_config,
                    )
                    .await
                    .map_err(ResolveSupergraphConfigError::ResolveSubgraphs)?;
                Ok(IntermediateSupergraphConfig {
                    state: Writing {
                        supergraph_config: resolved_supergraph_config.into(),
                    },
                })
            }
            None => Err(ResolveSupergraphConfigError::NoSource),
        }
    }

    pub async fn lazily_resolve_subgraphs(
        self,
        supergraph_config_root: &Utf8PathBuf,
    ) -> Result<IntermediateSupergraphConfig<Writing>, ResolveSupergraphConfigError> {
        match self.state.supergraph_config {
            Some(supergraph_config) => {
                let unresolved_supergraph_config =
                    UnresolvedSupergraphConfig::new(supergraph_config);
                let resolved_supergraph_config =
                    <ResolvedSupergraphConfig<LazilyResolvedSubgraph>>::resolve(
                        supergraph_config_root,
                        unresolved_supergraph_config,
                    )
                    .await
                    .map_err(ResolveSupergraphConfigError::ResolveSubgraphs)?;
                Ok(IntermediateSupergraphConfig {
                    state: Writing {
                        supergraph_config: resolved_supergraph_config.into(),
                    },
                })
            }
            None => Err(ResolveSupergraphConfigError::NoSource),
        }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum WriteSupergraphConfigError {
    #[error("Unable to serialized the supergraph config")]
    Serialization(#[from] serde_yaml::Error),
    #[error("Unable to write file")]
    Fs(RoverStdError),
}

impl IntermediateSupergraphConfig<Writing> {
    pub fn write(
        self,
        path: Utf8PathBuf,
    ) -> Result<FinalSupergraphConfig, WriteSupergraphConfigError> {
        let contents = serde_yaml::to_string(&self.state.supergraph_config)?;
        Fs::write_file(path.clone(), contents).map_err(WriteSupergraphConfigError::Fs)?;
        Ok(FinalSupergraphConfig {
            path,
            config: self.state.supergraph_config,
        })
    }
}

#[derive(Getters)]
pub struct FinalSupergraphConfig {
    path: Utf8PathBuf,
    #[getter(skip)]
    config: SupergraphConfig,
}

impl FinalSupergraphConfig {
    #[cfg(test)]
    pub fn new(path: Utf8PathBuf, config: SupergraphConfig) -> FinalSupergraphConfig {
        FinalSupergraphConfig { path, config }
    }
}

impl From<FinalSupergraphConfig> for SupergraphConfig {
    fn from(value: FinalSupergraphConfig) -> Self {
        value.config
    }
}

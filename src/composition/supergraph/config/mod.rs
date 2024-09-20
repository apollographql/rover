use std::sync::Arc;

use apollo_federation_types::config::{ConfigError, FederationVersion, SupergraphConfig};
use camino::Utf8PathBuf;
use derive_getters::Getters;
use rover_client::shared::GraphRef;
use rover_std::{Fs, RoverStdError};
use tokio::sync::{Mutex, MutexGuard};

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
    state::{LoadRemoteSubgraphs, ResolveSubgraphs, TargetFile},
};

mod remote_subgraphs;
pub mod resolve;

mod state {
    use apollo_federation_types::config::SupergraphConfig;
    use camino::Utf8PathBuf;

    pub struct LoadSupergraphConfig;
    pub struct LoadRemoteSubgraphs {
        pub origin_path: Option<Utf8PathBuf>,
        pub supergraph_config: Option<SupergraphConfig>,
    }
    pub struct ResolveSubgraphs {
        pub origin_path: Option<Utf8PathBuf>,
        pub supergraph_config: Option<SupergraphConfig>,
    }
    pub struct TargetFile {
        pub origin_path: Option<Utf8PathBuf>,
        pub supergraph_config: SupergraphConfig,
    }
}

use state::LoadSupergraphConfig;

pub struct SupergraphConfigResolver<State> {
    state: State,
}

impl SupergraphConfigResolver<LoadSupergraphConfig> {
    pub fn new() -> SupergraphConfigResolver<LoadSupergraphConfig> {
        SupergraphConfigResolver {
            state: LoadSupergraphConfig,
        }
    }
}

impl Default for SupergraphConfigResolver<LoadSupergraphConfig> {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(thiserror::Error, Debug)]
pub enum LoadSupergraphConfigError {
    #[error("Failed to parse the supergraph config")]
    SupergraphConfig(ConfigError),
    #[error("Failed to read file descriptor")]
    ReadFileDescriptor(RoverError),
}

impl SupergraphConfigResolver<LoadSupergraphConfig> {
    pub fn load_from_file_descriptor(
        self,
        file_descriptor_type: Option<&FileDescriptorType>,
    ) -> Result<SupergraphConfigResolver<LoadRemoteSubgraphs>, LoadSupergraphConfigError> {
        if let Some(file_descriptor_type) = file_descriptor_type {
            let supergraph_config = file_descriptor_type
                .read_file_descriptor("supergraph config", &mut std::io::stdin())
                .map_err(LoadSupergraphConfigError::ReadFileDescriptor)
                .and_then(|contents| {
                    SupergraphConfig::new_from_yaml(&contents)
                        .map_err(LoadSupergraphConfigError::SupergraphConfig)
                })?;
            let origin_path = match file_descriptor_type {
                FileDescriptorType::File(file) => Some(file.clone()),
                FileDescriptorType::Stdin => None,
            };
            Ok(SupergraphConfigResolver {
                state: LoadRemoteSubgraphs {
                    origin_path,
                    supergraph_config: Some(supergraph_config),
                },
            })
        } else {
            Ok(SupergraphConfigResolver {
                state: LoadRemoteSubgraphs {
                    origin_path: None,
                    supergraph_config: None,
                },
            })
        }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum LoadRemoteSubgraphsError {
    #[error(transparent)]
    FetchRemoteSubgraphsError(Box<dyn std::error::Error + Send + Sync>),
}

impl SupergraphConfigResolver<LoadRemoteSubgraphs> {
    pub async fn load_remote_subgraphs(
        self,
        fetch_remote_subgraphs_impl: &impl FetchRemoteSubgraphs,
        graph_ref: Option<&GraphRef>,
    ) -> Result<SupergraphConfigResolver<ResolveSubgraphs>, LoadRemoteSubgraphsError> {
        if let Some(graph_ref) = graph_ref {
            let remote_supergraph_config = fetch_remote_subgraphs_impl
                .fetch_remote_subgraphs(graph_ref)
                .await
                .map_err(|err| {
                    LoadRemoteSubgraphsError::FetchRemoteSubgraphsError(Box::new(err))
                })?;
            Ok(SupergraphConfigResolver {
                state: ResolveSubgraphs {
                    origin_path: self.state.origin_path,
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
            Ok(SupergraphConfigResolver {
                state: ResolveSubgraphs {
                    origin_path: self.state.origin_path,
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

impl SupergraphConfigResolver<ResolveSubgraphs> {
    pub async fn fully_resolve_subgraphs<CTX>(
        self,
        introspect_subgraph_impl: &impl IntrospectSubgraph,
        fetch_remote_subgraph_impl: &impl FetchRemoteSubgraph,
        supergraph_config_root: &Utf8PathBuf,
    ) -> Result<SupergraphConfigResolver<TargetFile>, ResolveSupergraphConfigError>
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
                Ok(SupergraphConfigResolver {
                    state: TargetFile {
                        origin_path: self.state.origin_path,
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
    ) -> Result<SupergraphConfigResolver<TargetFile>, ResolveSupergraphConfigError> {
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
                Ok(SupergraphConfigResolver {
                    state: TargetFile {
                        origin_path: self.state.origin_path,
                        supergraph_config: resolved_supergraph_config.into(),
                    },
                })
            }
            None => Err(ResolveSupergraphConfigError::NoSource),
        }
    }
}

impl SupergraphConfigResolver<TargetFile> {
    pub fn with_target(self, path: Utf8PathBuf) -> FinalSupergraphConfig {
        FinalSupergraphConfig {
            origin_path: self.state.origin_path,
            target_file: Arc::new(Mutex::new(path)),
            config: self.state.supergraph_config,
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

#[derive(Clone, Debug, Getters)]
pub struct FinalSupergraphConfig {
    origin_path: Option<Utf8PathBuf>,
    #[getter(skip)]
    target_file: Arc<Mutex<Utf8PathBuf>>,
    #[getter(skip)]
    config: SupergraphConfig,
}

impl FinalSupergraphConfig {
    #[cfg(test)]
    pub fn new(
        origin_path: Option<Utf8PathBuf>,
        target_file: Utf8PathBuf,
        config: SupergraphConfig,
    ) -> FinalSupergraphConfig {
        FinalSupergraphConfig {
            config,
            origin_path,
            target_file: Arc::new(Mutex::new(target_file)),
        }
    }

    pub async fn read_lock(&self) -> MutexGuard<Utf8PathBuf> {
        self.target_file.lock().await
    }

    pub async fn write(&self) -> Result<(), WriteSupergraphConfigError> {
        let target_file = self.target_file.lock().await;
        let contents = serde_yaml::to_string(&self.config)?;
        Fs::write_file(&*target_file, contents).map_err(WriteSupergraphConfigError::Fs)?;
        Ok(())
    }

    pub fn federation_version(&self) -> FederationVersion {
        self.config
            .get_federation_version()
            .unwrap_or(FederationVersion::LatestFedTwo)
    }
}

impl From<FinalSupergraphConfig> for SupergraphConfig {
    fn from(value: FinalSupergraphConfig) -> Self {
        value.config
    }
}

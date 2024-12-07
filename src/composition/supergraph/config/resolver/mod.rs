//! This module provides an object that can either produce a [`LazilyResolvedsupergraphConfig`] or a
//! [`FullyResolvedSupergraphConfig`] and uses the typestate pattern to enforce the order
//! in which certain steps must happen.
//!
//! The process that is outlined by this pattern is the following:
//!   1. Load remote subgraphs (if a [`GraphRef`] is provided)
//!   2. Load subgraphs from local config (if a supergraph config file is provided)
//!   3. Resolve subgraphs into one of: [`LazilyResolvedsupergraphConfig`] or [`FullyResolvedSupergraphConfig`]
//!      a. [`LazilyResolvedsupergraphConfig`] is used to spin up a [`SubgraphWatchers`] object, which
//!         provides SDL updates as subgraphs change
//!      b. [`FullyResolvedSupergraphConfig`] is used to produce a composition result
//!         from [`SupergraphBinary`]. This must be written to a file first, using the format defined
//!         by [`SupergraphConfig`]

use std::collections::BTreeMap;

use apollo_federation_types::config::{
    ConfigError, FederationVersion, SubgraphConfig, SupergraphConfig,
};
use camino::Utf8PathBuf;
use rover_client::shared::GraphRef;

use crate::{
    utils::{
        effect::{
            fetch_remote_subgraph::FetchRemoteSubgraph,
            fetch_remote_subgraphs::FetchRemoteSubgraphs, introspect::IntrospectSubgraph,
            read_stdin::ReadStdin,
        },
        parsers::FileDescriptorType,
    },
    RoverError,
};

use self::state::ResolveSubgraphs;

use super::{
    error::ResolveSubgraphError,
    federation::{
        FederationVersionMismatch, FederationVersionResolver,
        FederationVersionResolverFromSupergraphConfig,
    },
    full::FullyResolvedSupergraphConfig,
    lazy::LazilyResolvedSupergraphConfig,
    unresolved::UnresolvedSupergraphConfig,
};

mod state;

/// This is a state-based resolver for the different stages of resolving a supergraph config
pub struct SupergraphConfigResolver<State> {
    state: State,
}

impl SupergraphConfigResolver<state::LoadRemoteSubgraphs> {
    /// Creates a new [`SupergraphConfigResolver`] using a target federation Version
    pub fn new(
        federation_version: FederationVersion,
    ) -> SupergraphConfigResolver<state::LoadRemoteSubgraphs> {
        SupergraphConfigResolver {
            state: state::LoadRemoteSubgraphs {
                federation_version_resolver: FederationVersionResolverFromSupergraphConfig::new(
                    federation_version,
                ),
            },
        }
    }
}

impl Default for SupergraphConfigResolver<state::LoadRemoteSubgraphs> {
    fn default() -> Self {
        SupergraphConfigResolver {
            state: state::LoadRemoteSubgraphs {
                federation_version_resolver: FederationVersionResolver::default(),
            },
        }
    }
}

/// Errors that may occur when loading remote subgraphs
#[derive(thiserror::Error, Debug)]
pub enum LoadRemoteSubgraphsError {
    /// Error captured by the underlying implementation of [`FetchRemoteSubgraphs`]
    #[error(transparent)]
    FetchRemoteSubgraphsError(Box<dyn std::error::Error + Send + Sync>),
}

impl SupergraphConfigResolver<state::LoadRemoteSubgraphs> {
    /// Optionally loads subgraphs from the Studio API using the contents of the `--graph-ref` flag
    /// and an implementation of [`FetchRemoteSubgraphs`]
    pub async fn load_remote_subgraphs(
        self,
        fetch_remote_subgraphs_impl: &impl FetchRemoteSubgraphs,
        graph_ref: Option<&GraphRef>,
    ) -> Result<SupergraphConfigResolver<state::LoadSupergraphConfig>, LoadRemoteSubgraphsError>
    {
        if let Some(graph_ref) = graph_ref {
            let remote_subgraphs = fetch_remote_subgraphs_impl
                .fetch_remote_subgraphs(graph_ref)
                .await
                .map_err(|err| {
                    LoadRemoteSubgraphsError::FetchRemoteSubgraphsError(Box::new(err))
                })?;
            Ok(SupergraphConfigResolver {
                state: state::LoadSupergraphConfig {
                    federation_version_resolver: self.state.federation_version_resolver,
                    subgraphs: remote_subgraphs,
                },
            })
        } else {
            Ok(SupergraphConfigResolver {
                state: state::LoadSupergraphConfig {
                    federation_version_resolver: self.state.federation_version_resolver,
                    subgraphs: BTreeMap::default(),
                },
            })
        }
    }
}

/// Errors that may occur as a result of loading a local supergraph config
#[derive(thiserror::Error, Debug)]
pub enum LoadSupergraphConfigError {
    /// Occurs when a supergraph cannot be parsed as YAML
    #[error("Failed to parse the supergraph config. Error: {0}")]
    SupergraphConfig(ConfigError),
    /// IO error that occurs when the supergraph contents can't be access via File IO or Stdin
    #[error("Failed to read file descriptor. Error: {0}")]
    ReadFileDescriptor(RoverError),
}

impl SupergraphConfigResolver<state::LoadSupergraphConfig> {
    /// Optionally loads the file from a specified [`FileDescriptorType`], using the implementation
    /// of [`ReadStdin`] in cases where `file_descriptor_type` is specified and points at stdin
    pub fn load_from_file_descriptor(
        self,
        read_stdin_impl: &mut impl ReadStdin,
        file_descriptor_type: Option<&FileDescriptorType>,
    ) -> Result<SupergraphConfigResolver<ResolveSubgraphs>, LoadSupergraphConfigError> {
        if let Some(file_descriptor_type) = file_descriptor_type {
            let supergraph_config = file_descriptor_type
                .read_file_descriptor("supergraph config", read_stdin_impl)
                .map_err(LoadSupergraphConfigError::ReadFileDescriptor)
                .and_then(|contents| {
                    SupergraphConfig::new_from_yaml(&contents)
                        .map_err(LoadSupergraphConfigError::SupergraphConfig)
                })?;
            let origin_path = match file_descriptor_type {
                FileDescriptorType::File(file) => Some(file.clone()),
                FileDescriptorType::Stdin => None,
            };
            let federation_version_resolver = self
                .state
                .federation_version_resolver
                .from_supergraph_config(Some(&supergraph_config));
            let mut merged_subgraphs = self.state.subgraphs;
            for (name, subgraph_config) in supergraph_config.into_iter() {
                let subgraph_config = SubgraphConfig {
                    routing_url: subgraph_config.routing_url.or_else(|| {
                        merged_subgraphs
                            .get(&name)
                            .and_then(|remote_config| remote_config.routing_url.clone())
                    }),
                    schema: subgraph_config.schema,
                };
                merged_subgraphs.insert(name, subgraph_config);
            }
            Ok(SupergraphConfigResolver {
                state: ResolveSubgraphs {
                    origin_path,
                    federation_version_resolver,
                    subgraphs: merged_subgraphs,
                },
            })
        } else {
            Ok(SupergraphConfigResolver {
                state: ResolveSubgraphs {
                    origin_path: None,
                    federation_version_resolver: self
                        .state
                        .federation_version_resolver
                        .from_supergraph_config(None),
                    subgraphs: self.state.subgraphs,
                },
            })
        }
    }
}

/// Errors that may occur while resolving a supergraph config
#[derive(thiserror::Error, Debug)]
pub enum ResolveSupergraphConfigError {
    /// Occurs when the caller neither loads a remote supergraph config nor a local one
    #[error("No source found for supergraph config")]
    NoSource,
    /// Occurs when the underlying resolver strategy can't resolve one or more
    /// of the subgraphs described in the supergraph config
    #[error("Unable to resolve subgraphs")]
    ResolveSubgraphs(Vec<ResolveSubgraphError>),
    /// Occurs when the user-selected `FederationVersion` is within Federation 1 boundaries, but the
    /// subgraphs use the `@link` directive, which requires Federation 2
    #[error(transparent)]
    FederationVersionMismatch(#[from] FederationVersionMismatch),
}

impl SupergraphConfigResolver<ResolveSubgraphs> {
    /// Fully resolves the subgraph configurations in the supergraph config file to their SDLs
    pub async fn fully_resolve_subgraphs(
        &self,
        introspect_subgraph_impl: &impl IntrospectSubgraph,
        fetch_remote_subgraph_impl: &impl FetchRemoteSubgraph,
        supergraph_config_root: Option<&Utf8PathBuf>,
    ) -> Result<FullyResolvedSupergraphConfig, ResolveSupergraphConfigError> {
        if !self.state.subgraphs.is_empty() {
            let unresolved_supergraph_config = UnresolvedSupergraphConfig::builder()
                .subgraphs(self.state.subgraphs.clone())
                .federation_version_resolver(self.state.federation_version_resolver.clone())
                .build();
            let resolved_supergraph_config = FullyResolvedSupergraphConfig::resolve(
                introspect_subgraph_impl,
                fetch_remote_subgraph_impl,
                supergraph_config_root,
                unresolved_supergraph_config,
            )
            .await?;
            Ok(resolved_supergraph_config)
        } else {
            Err(ResolveSupergraphConfigError::NoSource)
        }
    }

    /// Resolves the subgraph configurations in the supergraph config file such that their file paths
    /// are valid and relative to the supergraph config file (or working directory, if the supergraph
    /// config is piped through stdin
    pub async fn lazily_resolve_subgraphs(
        &self,
        supergraph_config_root: &Utf8PathBuf,
    ) -> Result<LazilyResolvedSupergraphConfig, ResolveSupergraphConfigError> {
        if !self.state.subgraphs.is_empty() {
            let unresolved_supergraph_config = UnresolvedSupergraphConfig::builder()
                .subgraphs(self.state.subgraphs.clone())
                .federation_version_resolver(self.state.federation_version_resolver.clone())
                .build();
            let resolved_supergraph_config = LazilyResolvedSupergraphConfig::resolve(
                supergraph_config_root,
                unresolved_supergraph_config,
            )
            .await
            .map_err(ResolveSupergraphConfigError::ResolveSubgraphs)?;
            Ok(resolved_supergraph_config)
        } else {
            Err(ResolveSupergraphConfigError::NoSource)
        }
    }
}

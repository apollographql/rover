use std::collections::BTreeMap;

use apollo_federation_types::config::{FederationVersion, SupergraphConfig};
use camino::Utf8PathBuf;
use derive_getters::Getters;
use futures::{stream, StreamExt, TryFutureExt};
use itertools::Itertools;
use tower::MakeService;

use crate::{
    composition::supergraph::config::{
        error::ResolveSubgraphError,
        resolver::{
            fetch_remote_subgraph::{FetchRemoteSubgraphRequest, RemoteSubgraph},
            ResolveSupergraphConfigError,
        },
        unresolved::UnresolvedSupergraphConfig,
    },
    utils::effect::introspect::IntrospectSubgraph,
};

use super::FullyResolvedSubgraph;

/// Represents a [`SupergraphConfig`] that has a known [`FederationVersion`] and
/// its subgraph [`SchemaSource`]s reduced to [`SchemaSource::Sdl`]
#[derive(Clone, Debug, Eq, PartialEq, Getters)]
#[cfg_attr(test, derive(buildstructor::Builder))]
pub struct FullyResolvedSupergraphConfig {
    origin_path: Option<Utf8PathBuf>,
    subgraphs: BTreeMap<String, FullyResolvedSubgraph>,
    federation_version: FederationVersion,
}

impl From<FullyResolvedSupergraphConfig> for SupergraphConfig {
    fn from(value: FullyResolvedSupergraphConfig) -> Self {
        let subgraphs = value
            .subgraphs
            .into_iter()
            .map(|(name, subgraph)| (name, subgraph.into()))
            .collect();
        SupergraphConfig::new(subgraphs, Some(value.federation_version))
    }
}

impl FullyResolvedSupergraphConfig {
    /// Resolves an [`UnresolvedSupergraphConfig`] into a [`FullyResolvedSupergraphConfig`]
    /// by resolving the individual subgraphs concurrently and calculating the [`FederationVersion`]
    pub async fn resolve<MakeFetchSubgraph>(
        introspect_subgraph_impl: &impl IntrospectSubgraph,
        fetch_remote_subgraph_impl: MakeFetchSubgraph,
        supergraph_config_root: Option<&Utf8PathBuf>,
        unresolved_supergraph_config: UnresolvedSupergraphConfig,
    ) -> Result<FullyResolvedSupergraphConfig, ResolveSupergraphConfigError>
    where
        MakeFetchSubgraph:
            MakeService<(), FetchRemoteSubgraphRequest, Response = RemoteSubgraph> + Clone,
        MakeFetchSubgraph::MakeError: std::error::Error + Send + Sync + 'static,
        MakeFetchSubgraph::Error: std::error::Error + Send + Sync + 'static,
    {
        let subgraphs = stream::iter(unresolved_supergraph_config.subgraphs().iter().map(
            move |(name, unresolved_subgraph)| {
                let fetch_remote_subgraph_impl = fetch_remote_subgraph_impl.clone();
                FullyResolvedSubgraph::resolve(
                    introspect_subgraph_impl,
                    fetch_remote_subgraph_impl,
                    supergraph_config_root,
                    unresolved_subgraph.clone(),
                )
                .map_ok(|result| (name.to_string(), result))
            },
        ))
        .buffer_unordered(50)
        .collect::<Vec<Result<(String, FullyResolvedSubgraph), ResolveSubgraphError>>>()
        .await;
        let (subgraphs, errors): (
            Vec<(String, FullyResolvedSubgraph)>,
            Vec<ResolveSubgraphError>,
        ) = subgraphs.into_iter().partition_result();
        if errors.is_empty() {
            let subgraphs = BTreeMap::from_iter(subgraphs);
            let federation_version = unresolved_supergraph_config
                .federation_version_resolver()
                .clone()
                .ok_or_else(|| ResolveSupergraphConfigError::MissingFederationVersionResolver)?
                .resolve(subgraphs.iter())?;
            Ok(FullyResolvedSupergraphConfig {
                origin_path: unresolved_supergraph_config.origin_path().clone(),
                subgraphs,
                federation_version,
            })
        } else {
            Err(ResolveSupergraphConfigError::ResolveSubgraphs(errors))
        }
    }

    /// Updates the subgraph with the provided name using the provided schema
    pub fn update_subgraph_schema(&mut self, name: String, subgraph: FullyResolvedSubgraph) {
        self.subgraphs.insert(name, subgraph);
    }

    /// Removes the subgraph with the name provided
    pub fn remove_subgraph(&mut self, name: &str) {
        self.subgraphs.remove(name);
    }
}

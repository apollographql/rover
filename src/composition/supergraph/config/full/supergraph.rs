use std::collections::BTreeMap;

use apollo_federation_types::config::{FederationVersion, SupergraphConfig};
use camino::Utf8PathBuf;
use derive_getters::Getters;
use futures::{stream, StreamExt, TryFutureExt};
use itertools::Itertools;
use rover_http::HttpService;
use tower::{MakeService, Service, ServiceExt};

use crate::composition::supergraph::config::{
    error::ResolveSubgraphError,
    resolver::{
        fetch_remote_subgraph::{FetchRemoteSubgraphRequest, RemoteSubgraph},
        ResolveSupergraphConfigError,
    },
    unresolved::UnresolvedSupergraphConfig,
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
    pub async fn resolve<MakeFetchSubgraph, FetchSubgraph>(
        http_service: HttpService,
        fetch_remote_subgraph_impl: MakeFetchSubgraph,
        supergraph_config_root: &Utf8PathBuf,
        unresolved_supergraph_config: UnresolvedSupergraphConfig,
    ) -> Result<FullyResolvedSupergraphConfig, ResolveSupergraphConfigError>
    where
        MakeFetchSubgraph: MakeService<
                (),
                FetchRemoteSubgraphRequest,
                Response = RemoteSubgraph,
                Service = FetchSubgraph,
            > + Clone,
        MakeFetchSubgraph::MakeError: std::error::Error + Send + Sync + 'static,
        MakeFetchSubgraph::Error: std::error::Error + Send + Sync + 'static,
        FetchSubgraph:
            Service<FetchRemoteSubgraphRequest, Response = RemoteSubgraph> + Clone + Send + 'static,
        FetchSubgraph::Error: std::error::Error + Send + Sync + 'static,
        FetchSubgraph::Future: Send,
    {
        let subgraphs = stream::iter(unresolved_supergraph_config.subgraphs().iter().map(
            move |(name, unresolved_subgraph)| {
                let fetch_remote_subgraph_impl = fetch_remote_subgraph_impl.clone();
                FullyResolvedSubgraph::resolver(
                    http_service.clone(),
                    fetch_remote_subgraph_impl,
                    supergraph_config_root,
                    unresolved_subgraph.clone(),
                )
                .map_err(|err| (name.to_string(), err))
                .and_then(|service| {
                    let mut service = service.clone();
                    let name = name.to_string();
                    async move {
                        let service = service
                            .ready()
                            .await
                            .map_err(|err| (name.to_string(), err))?;
                        let result = service
                            .call(())
                            .await
                            .map_err(|err| (name.to_string(), err))?;
                        Ok((name.to_string(), result))
                    }
                })
            },
        ))
        .buffer_unordered(50)
        .collect::<Vec<Result<(String, FullyResolvedSubgraph), (String, ResolveSubgraphError)>>>()
        .await;
        #[allow(clippy::type_complexity)]
        let (subgraphs, errors): (
            Vec<(String, FullyResolvedSubgraph)>,
            Vec<(String, ResolveSubgraphError)>,
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
            Err(ResolveSupergraphConfigError::ResolveSubgraphs(
                BTreeMap::from_iter(errors.into_iter()),
            ))
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

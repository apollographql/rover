use std::collections::BTreeMap;

use apollo_federation_types::config::FederationVersion;
use camino::Utf8PathBuf;
use derive_getters::Getters;
use futures::{StreamExt, stream};
use itertools::Itertools;
use tower::{Service, ServiceExt};
use tracing::debug;

use super::FullyResolvedSubgraph;
use crate::composition::supergraph::config::SupergraphConfigYaml;
use crate::composition::supergraph::config::error::ResolveSubgraphError;
use crate::composition::supergraph::config::full::introspect::ResolveIntrospectSubgraphFactory;
use crate::composition::supergraph::config::resolver::ResolveSupergraphConfigError;
use crate::composition::supergraph::config::resolver::fetch_remote_subgraph::FetchRemoteSubgraphFactory;
use crate::composition::supergraph::config::unresolved::{
    UnresolvedSubgraph, UnresolvedSupergraphConfig,
};

/// Represents a [`SupergraphConfigYaml`] that has a known [`FederationVersion`] and
/// its subgraph [`SchemaSource`]s reduced to [`SchemaSource::Sdl`]
#[derive(Clone, Debug, Eq, PartialEq, Getters)]
#[cfg_attr(test, derive(buildstructor::Builder))]
pub struct FullyResolvedSupergraphConfig {
    pub(crate) origin_path: Option<Utf8PathBuf>,
    pub(crate) subgraphs: BTreeMap<String, FullyResolvedSubgraph>,
    pub(crate) federation_version: FederationVersion,
}

impl From<FullyResolvedSupergraphConfig> for SupergraphConfigYaml {
    fn from(value: FullyResolvedSupergraphConfig) -> Self {
        let subgraphs = value
            .subgraphs
            .into_iter()
            .map(|(name, subgraph)| (name, subgraph.into()))
            .collect();
        SupergraphConfigYaml {
            subgraphs,
            federation_version: Some(value.federation_version),
        }
    }
}

impl FullyResolvedSupergraphConfig {
    /// Resolves an [`UnresolvedSupergraphConfig`] into a [`FullyResolvedSupergraphConfig`]
    /// by resolving the individual subgraphs concurrently and calculating the [`FederationVersion`]
    pub async fn resolve(
        resolve_introspect_subgraph_factory: ResolveIntrospectSubgraphFactory,
        fetch_remote_subgraph_factory: FetchRemoteSubgraphFactory,
        supergraph_config_root: &Utf8PathBuf,
        unresolved_supergraph_config: UnresolvedSupergraphConfig,
    ) -> Result<
        (
            FullyResolvedSupergraphConfig,
            BTreeMap<String, ResolveSubgraphError>,
        ),
        ResolveSupergraphConfigError,
    > {
        let subgraphs = stream::iter(unresolved_supergraph_config.subgraphs.into_iter().map(
            move |(name, subgraph)| {
                let fetch_remote_subgraph_factory = fetch_remote_subgraph_factory.clone();
                let resolve_introspect_subgraph_factory =
                    resolve_introspect_subgraph_factory.clone();
                async move {
                    match FullyResolvedSubgraph::resolver(
                        resolve_introspect_subgraph_factory,
                        fetch_remote_subgraph_factory,
                        supergraph_config_root,
                        UnresolvedSubgraph {
                            schema: subgraph.schema,
                            routing_url: subgraph.routing_url,
                            name: name.clone(),
                        },
                    )
                    .await
                    {
                        Ok(mut service) => {
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
                        Err(err) => Err((name.to_string(), err)),
                    }
                }
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
        let subgraphs = BTreeMap::from_iter(subgraphs);
        let federation_version = unresolved_supergraph_config
            .federation_version_resolver
            .ok_or_else(|| ResolveSupergraphConfigError::MissingFederationVersionResolver)?
            .resolve(subgraphs.iter())?;
        Ok((
            FullyResolvedSupergraphConfig {
                origin_path: unresolved_supergraph_config.origin_path.clone(),
                subgraphs,
                federation_version,
            },
            BTreeMap::from_iter(errors.into_iter()),
        ))
    }

    /// Updates the subgraph with the provided name using the provided schema
    pub fn update_subgraph_schema(
        &mut self,
        name: String,
        subgraph: FullyResolvedSubgraph,
    ) -> Option<FullyResolvedSubgraph> {
        self.subgraphs.insert(name, subgraph)
    }

    pub(crate) fn update_routing_url(
        &mut self,
        subgraph_name: &str,
        routing_url: Option<String>,
    ) -> Option<Option<String>> {
        match self.subgraphs.get_mut(subgraph_name) {
            None => {
                debug!("Could not find subgraph {}", subgraph_name);
                None
            }
            Some(subgraph) => {
                let original_value = subgraph.routing_url.clone();
                if routing_url != subgraph.routing_url {
                    debug!(
                        "Updating routing URL from {:?} to {:?}",
                        subgraph.routing_url, routing_url
                    );
                    subgraph.routing_url = routing_url;
                    Some(original_value)
                } else {
                    None
                }
            }
        }
    }

    /// Removes the subgraph with the name provided
    pub fn remove_subgraph(&mut self, name: &str) {
        self.subgraphs.remove(name);
    }
}

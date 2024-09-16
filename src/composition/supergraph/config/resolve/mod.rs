use std::collections::BTreeMap;

use apollo_federation_types::config::{FederationVersion, SubgraphConfig, SupergraphConfig};
use camino::Utf8PathBuf;
use futures::{
    stream::{self, StreamExt},
    TryFutureExt,
};
use itertools::Itertools;

use crate::utils::effect::{
    fetch_remote_subgraph::FetchRemoteSubgraph, introspect::IntrospectSubgraph,
};

use self::subgraph::{
    FullyResolvedSubgraph, LazilyResolvedSubgraph, ResolveSubgraphError, UnresolvedSubgraph,
};

pub mod subgraph;

pub struct UnresolvedSupergraphConfig {
    subgraphs: BTreeMap<String, UnresolvedSubgraph>,
    federation_version: FederationVersion,
}

impl UnresolvedSupergraphConfig {
    pub fn new(supergraph_config: SupergraphConfig) -> UnresolvedSupergraphConfig {
        let federation_version = supergraph_config
            .get_federation_version()
            .unwrap_or(FederationVersion::LatestFedTwo);
        let subgraphs = supergraph_config
            .into_iter()
            .map(|(name, subgraph)| (name.to_string(), UnresolvedSubgraph::new(name, subgraph)))
            .collect();
        UnresolvedSupergraphConfig {
            subgraphs,
            federation_version,
        }
    }
}

pub struct ResolvedSupergraphConfig<ResolvedSubgraph: Into<SubgraphConfig>> {
    subgraphs: BTreeMap<String, ResolvedSubgraph>,
    federation_version: FederationVersion,
}

impl<T: Into<SubgraphConfig>> From<ResolvedSupergraphConfig<T>> for SupergraphConfig {
    fn from(value: ResolvedSupergraphConfig<T>) -> Self {
        let subgraphs = value
            .subgraphs
            .into_iter()
            .map(|(name, subgraph)| (name, subgraph.into()))
            .collect();
        SupergraphConfig::new(subgraphs, Some(value.federation_version))
    }
}

impl ResolvedSupergraphConfig<FullyResolvedSubgraph> {
    pub async fn resolve(
        introspect_subgraph_impl: &impl IntrospectSubgraph,
        fetch_remote_subgraph_impl: &impl FetchRemoteSubgraph,
        supergraph_config_root: &Utf8PathBuf,
        unresolved_supergraph_config: UnresolvedSupergraphConfig,
    ) -> Result<ResolvedSupergraphConfig<FullyResolvedSubgraph>, Vec<ResolveSubgraphError>> {
        let subgraphs = stream::iter(unresolved_supergraph_config.subgraphs.into_iter().map(
            |(name, unresolved_subgraph)| {
                FullyResolvedSubgraph::resolve(
                    introspect_subgraph_impl,
                    fetch_remote_subgraph_impl,
                    supergraph_config_root,
                    unresolved_subgraph,
                )
                .map_ok(|result| (name, result))
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
            Ok(ResolvedSupergraphConfig {
                subgraphs: BTreeMap::from_iter(subgraphs),
                federation_version: unresolved_supergraph_config.federation_version,
            })
        } else {
            Err(errors)
        }
    }
}

impl ResolvedSupergraphConfig<LazilyResolvedSubgraph> {
    pub async fn resolve(
        supergraph_config_root: &Utf8PathBuf,
        unresolved_supergraph_config: UnresolvedSupergraphConfig,
    ) -> Result<ResolvedSupergraphConfig<LazilyResolvedSubgraph>, Vec<ResolveSubgraphError>> {
        let subgraphs = stream::iter(unresolved_supergraph_config.subgraphs.into_iter().map(
            |(name, unresolved_subgraph)| async {
                let result =
                    LazilyResolvedSubgraph::resolve(supergraph_config_root, unresolved_subgraph)?;
                Ok((name, result))
            },
        ))
        .buffer_unordered(50)
        .collect::<Vec<Result<(String, LazilyResolvedSubgraph), ResolveSubgraphError>>>()
        .await;
        let (subgraphs, errors): (
            Vec<(String, LazilyResolvedSubgraph)>,
            Vec<ResolveSubgraphError>,
        ) = subgraphs.into_iter().partition_result();
        if errors.is_empty() {
            Ok(ResolvedSupergraphConfig {
                subgraphs: BTreeMap::from_iter(subgraphs),
                federation_version: unresolved_supergraph_config.federation_version,
            })
        } else {
            Err(errors)
        }
    }
}

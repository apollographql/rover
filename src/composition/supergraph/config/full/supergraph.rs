use std::collections::BTreeMap;

use apollo_federation_types::config::{FederationVersion, SupergraphConfig};
use camino::Utf8PathBuf;
use derive_getters::Getters;
use futures::{stream, StreamExt, TryFutureExt};
use itertools::Itertools;

use crate::{
    composition::supergraph::config::{
        error::ResolveSubgraphError, resolver::ResolveSupergraphConfigError,
        unresolved::UnresolvedSupergraphConfig,
    },
    utils::effect::{fetch_remote_subgraph::FetchRemoteSubgraph, introspect::IntrospectSubgraph},
};

use super::FullyResolvedSubgraph;

/// Represents a [`SupergraphConfig`] that has a known [`FederationVersion`] and
/// its subgraph [`SchemaSource`]s reduced to [`SchemaSource::Sdl`]
#[derive(Clone, Debug, Eq, PartialEq, Getters)]
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
    pub async fn resolve(
        introspect_subgraph_impl: &impl IntrospectSubgraph,
        fetch_remote_subgraph_impl: &impl FetchRemoteSubgraph,
        supergraph_config_root: Option<&Utf8PathBuf>,
        unresolved_supergraph_config: UnresolvedSupergraphConfig,
    ) -> Result<FullyResolvedSupergraphConfig, ResolveSupergraphConfigError> {
        let subgraphs = stream::iter(unresolved_supergraph_config.subgraphs().iter().map(
            |(name, unresolved_subgraph)| {
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
            let federation_version = Self::resolve_federation_version(
                unresolved_supergraph_config.federation_version().as_ref(),
                &mut subgraphs.iter(),
            )?;
            Ok(FullyResolvedSupergraphConfig {
                origin_path: unresolved_supergraph_config.origin_path().clone(),
                subgraphs,
                federation_version,
            })
        } else {
            Err(ResolveSupergraphConfigError::ResolveSubgraphs(errors))
        }
    }

    fn resolve_federation_version<'a>(
        specified_federation_version: Option<&FederationVersion>,
        subgraphs: &'a mut impl Iterator<Item = (&'a String, &'a FullyResolvedSubgraph)>,
    ) -> Result<FederationVersion, ResolveSupergraphConfigError> {
        let fed_two_subgraphs = subgraphs
            .filter_map(|(subgraph_name, subgraph)| {
                if *subgraph.is_fed_two() {
                    Some(subgraph_name.to_string())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        let contains_fed_two_subgraphs = !fed_two_subgraphs.is_empty();
        match specified_federation_version {
            Some(specified_federation_version) => {
                let specified_federation_version = specified_federation_version.clone();
                if specified_federation_version.is_fed_one() {
                    if contains_fed_two_subgraphs {
                        Err(ResolveSupergraphConfigError::FederationVersionMismatch {
                            specified_federation_version,
                            subgraph_names: fed_two_subgraphs,
                        })
                    } else {
                        Ok(specified_federation_version)
                    }
                } else {
                    Ok(specified_federation_version)
                }
            }
            None => Ok(FederationVersion::LatestFedTwo),
        }
    }
}

use std::collections::BTreeMap;

use apollo_federation_types::config::{FederationVersion, SupergraphConfig};
use camino::Utf8PathBuf;
use derive_getters::Getters;
use futures::{stream, StreamExt};
use itertools::Itertools;

use super::LazilyResolvedSubgraph;
use crate::composition::supergraph::config::{
    error::ResolveSubgraphError, unresolved::UnresolvedSupergraphConfig,
};

/// Represents a [`SupergraphConfig`] where all its [`SchemaSource::File`] subgraphs have
/// known and valid file paths relative to a supergraph config file (or working directory of the
/// program, if the supergraph config is piped into stdin)
#[derive(Clone, Debug, Eq, PartialEq, Getters)]
pub struct LazilyResolvedSupergraphConfig {
    origin_path: Option<Utf8PathBuf>,
    subgraphs: BTreeMap<String, LazilyResolvedSubgraph>,
    federation_version: Option<FederationVersion>,
}

impl LazilyResolvedSupergraphConfig {
    /// Builds a new config, with the given options
    pub fn new(
        origin_path: Option<Utf8PathBuf>,
        subgraphs: BTreeMap<String, LazilyResolvedSubgraph>,
        federation_version: Option<FederationVersion>,
    ) -> Self {
        LazilyResolvedSupergraphConfig {
            origin_path,
            subgraphs,
            federation_version,
        }
    }

    /// Resolves an [`UnresolvedSupergraphConfig`] into a [`LazilyResolvedSupergraphConfig`] by
    /// making sure any internal file paths are correct
    pub async fn resolve(
        supergraph_config_root: &Utf8PathBuf,
        unresolved_supergraph_config: UnresolvedSupergraphConfig,
    ) -> (
        LazilyResolvedSupergraphConfig,
        BTreeMap<String, ResolveSubgraphError>,
    ) {
        let subgraphs = stream::iter(
            unresolved_supergraph_config
                .subgraphs()
                .clone()
                .into_iter()
                .map(|(name, unresolved_subgraph)| async move {
                    let result = LazilyResolvedSubgraph::resolve(
                        supergraph_config_root,
                        unresolved_subgraph.clone(),
                    )
                    .map_err(|err| (name.to_string(), err))?;
                    Ok((name.to_string(), result))
                }),
        )
        .buffer_unordered(50)
        .collect::<Vec<Result<(String, LazilyResolvedSubgraph), (String, ResolveSubgraphError)>>>()
        .await;
        #[allow(clippy::type_complexity)]
        let (subgraphs, errors): (
            Vec<(String, LazilyResolvedSubgraph)>,
            Vec<(String, ResolveSubgraphError)>,
        ) = subgraphs.into_iter().partition_result();
        (
            LazilyResolvedSupergraphConfig {
                origin_path: unresolved_supergraph_config.origin_path().clone(),
                subgraphs: BTreeMap::from_iter(subgraphs),
                federation_version: unresolved_supergraph_config.target_federation_version(),
            },
            BTreeMap::from_iter(errors.into_iter()),
        )
    }
}

impl From<LazilyResolvedSupergraphConfig> for SupergraphConfig {
    fn from(value: LazilyResolvedSupergraphConfig) -> Self {
        let subgraphs = BTreeMap::from_iter(
            value
                .subgraphs
                .into_iter()
                .map(|(name, subgraph)| (name, subgraph.into())),
        );
        SupergraphConfig::new(subgraphs, value.federation_version)
    }
}

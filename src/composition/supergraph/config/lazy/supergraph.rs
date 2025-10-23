use std::collections::BTreeMap;

use apollo_federation_types::config::FederationVersion;
use camino::Utf8PathBuf;
use derive_getters::Getters;
use futures::{StreamExt, stream};
use itertools::Itertools;

use super::LazilyResolvedSubgraph;
use crate::composition::supergraph::config::{
    SupergraphConfigYaml, error::ResolveSubgraphError, unresolved::UnresolvedSupergraphConfig,
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
    /// Resolves an [`UnresolvedSupergraphConfig`] into a [`LazilyResolvedSupergraphConfig`] by
    /// making sure any internal file paths are correct
    pub async fn resolve(
        supergraph_config_root: &Utf8PathBuf,
        unresolved_supergraph_config: UnresolvedSupergraphConfig,
    ) -> (
        LazilyResolvedSupergraphConfig,
        BTreeMap<String, ResolveSubgraphError>,
    ) {
        let federation_version = unresolved_supergraph_config.target_federation_version();
        let subgraphs = stream::iter(unresolved_supergraph_config.subgraphs.into_iter().map(
            |(name, unresolved_subgraph)| async move {
                match LazilyResolvedSubgraph::resolve(
                    supergraph_config_root,
                    name.clone(),
                    unresolved_subgraph,
                ) {
                    Ok(result) => Ok((name, result)),
                    Err(err) => Err((name, err)),
                }
            },
        ))
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
                origin_path: unresolved_supergraph_config.origin_path.clone(),
                subgraphs: BTreeMap::from_iter(subgraphs),
                federation_version,
            },
            BTreeMap::from_iter(errors.into_iter()),
        )
    }

    /// Updates the internal structure of the SupergraphConfig by filtering out
    /// any subgraphs that are included in the list of subgraphs to remove.
    pub fn filter_subgraphs(&mut self, subgraphs_to_remove: Vec<String>) {
        self.subgraphs
            .retain(|name, _| !subgraphs_to_remove.contains(name));
    }
}

impl From<LazilyResolvedSupergraphConfig> for SupergraphConfigYaml {
    fn from(value: LazilyResolvedSupergraphConfig) -> Self {
        let subgraphs = BTreeMap::from_iter(
            value
                .subgraphs
                .into_iter()
                .map(|(name, subgraph)| (name, subgraph.into())),
        );
        SupergraphConfigYaml {
            subgraphs,
            federation_version: value.federation_version,
        }
    }
}

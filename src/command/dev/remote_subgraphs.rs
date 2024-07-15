use apollo_federation_types::config::{
    FederationVersion, SchemaSource, SubgraphConfig, SupergraphConfig,
};
use rover_client::{
    blocking::StudioClient,
    operations::subgraph::{self, fetch_all::SubgraphFetchAllInput},
    shared::GraphRef,
};

use crate::RoverResult;

/// Nominal type that captures the behavior of collecting remote subgraphs into a
/// [`SupergraphConfig`] representation
#[derive(Clone, Debug)]
pub struct RemoteSubgraphs(SupergraphConfig);

impl RemoteSubgraphs {
    /// Fetches [`RemoteSubgraphs`] from Studio
    pub fn fetch(
        client: &StudioClient,
        federation_version: &FederationVersion,
        graph_ref: &GraphRef,
    ) -> RoverResult<RemoteSubgraphs> {
        let subgraphs = subgraph::fetch_all::run(
            SubgraphFetchAllInput {
                graph_ref: graph_ref.clone(),
            },
            client,
        )?;
        let subgraphs = subgraphs
            .iter()
            .map(|subgraph| {
                (
                    subgraph.name().clone(),
                    SubgraphConfig {
                        routing_url: subgraph.url().clone(),
                        schema: SchemaSource::Sdl {
                            sdl: subgraph.sdl().clone(),
                        },
                    },
                )
            })
            .collect();
        let supergraph_config = SupergraphConfig::new(subgraphs, Some(federation_version.clone()));
        let remote_subgraphs = RemoteSubgraphs(supergraph_config);
        Ok(remote_subgraphs)
    }

    /// Provides a reference to the inner value of this representation
    pub fn inner(&self) -> &SupergraphConfig {
        &self.0
    }
}

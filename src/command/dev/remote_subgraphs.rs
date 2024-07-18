use apollo_federation_types::config::{
    FederationVersion, SchemaSource, SubgraphConfig, SupergraphConfig,
};
use rover_client::{
    blocking::StudioClient,
    operations::subgraph::{self, list::SubgraphListInput},
    shared::GraphRef,
};

use crate::RoverResult;

/// Nominal type that captures the behavior of collecting remote subgraphs into a
/// [`SupergraphConfig`] representation
#[derive(Clone, Debug)]
pub struct RemoteSubgraphs(SupergraphConfig);

impl RemoteSubgraphs {
    /// Fetches [`RemoteSubgraphs`] from Studio
    pub async fn fetch(
        client: &StudioClient,
        federation_version: &FederationVersion,
        graph_ref: &GraphRef,
    ) -> RoverResult<RemoteSubgraphs> {
        let subgraphs = subgraph::list::run(
            SubgraphListInput {
                graph_ref: graph_ref.clone(),
            },
            client,
        )
        .await?;
        let subgraphs = subgraphs
            .subgraphs
            .iter()
            .map(|subgraph| {
                (
                    subgraph.name.clone(),
                    SubgraphConfig {
                        routing_url: subgraph.url.clone(),
                        schema: SchemaSource::Subgraph {
                            graphref: graph_ref.clone().to_string(),
                            subgraph: subgraph.name.clone(),
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

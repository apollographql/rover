use std::collections::BTreeMap;

use apollo_federation_types::config::SubgraphConfig;
use async_trait::async_trait;
use rover_client::{
    blocking::StudioClient,
    operations::subgraph::{
        self,
        fetch_all::{SubgraphFetchAllInput, SubgraphFetchAllResponse},
    },
    shared::GraphRef,
    RoverClientError,
};

#[cfg_attr(test, derive(thiserror::Error, Debug))]
#[cfg(test)]
#[cfg_attr(test, error("MockFetchRemoteSubgraphsError"))]
pub struct MockFetchRemoteSubgraphsError {}

#[cfg_attr(test, mockall::automock(type Error = MockFetchRemoteSubgraphsError;))]
#[async_trait]
pub trait FetchRemoteSubgraphs {
    type Error: std::error::Error + Send + Sync + 'static;
    async fn fetch_remote_subgraphs(
        &self,
        graph_ref: &GraphRef,
    ) -> Result<BTreeMap<String, SubgraphConfig>, Self::Error>;
}

#[async_trait]
impl FetchRemoteSubgraphs for StudioClient {
    type Error = RoverClientError;
    /// Fetches [`RemoteSubgraphs`] from Studio
    async fn fetch_remote_subgraphs(
        &self,
        graph_ref: &GraphRef,
    ) -> Result<BTreeMap<String, SubgraphConfig>, Self::Error> {
        let SubgraphFetchAllResponse { subgraphs, .. } = subgraph::fetch_all::run(
            SubgraphFetchAllInput {
                graph_ref: graph_ref.clone(),
            },
            self,
        )
        .await?;
        let subgraphs = subgraphs
            .into_iter()
            .map(|subgraph| (subgraph.name().clone(), subgraph.into()))
            .collect();
        Ok(subgraphs)
    }
}

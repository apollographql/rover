use async_trait::async_trait;
use buildstructor::Builder;
use derive_getters::Getters;
use rover_client::{
    blocking::StudioClient,
    operations::subgraph::fetch::{self, SubgraphFetchInput},
    shared::{GraphRef, SdlType},
    RoverClientError,
};

use crate::RoverError;

#[derive(Builder, Getters)]
pub struct RemoteSubgraph {
    name: String,
    routing_url: String,
    schema: String,
}

#[cfg_attr(test, derive(thiserror::Error, Debug))]
#[cfg_attr(test, error("{}", .0))]
#[cfg(test)]
pub struct MockFetchRemoteSubgraphError(String);

#[cfg_attr(test, mockall::automock(type Error = MockFetchRemoteSubgraphError;))]
#[async_trait]
pub trait FetchRemoteSubgraph {
    type Error: std::error::Error + 'static;
    async fn fetch_remote_subgraph(
        &self,
        graph_ref: GraphRef,
        subgraph_name: String,
    ) -> Result<RemoteSubgraph, Self::Error>;
}

#[derive(thiserror::Error, Debug)]
pub enum StudioFetchRemoteSdlError {
    #[error("Failed to build the client")]
    Reqwest(RoverError),
    #[error("Failed to fetch the subgraph from remote")]
    FetchSubgraph(#[from] RoverClientError),
    #[error("Got an invalid SDL type: {:?}", .0)]
    InvalidSdlType(SdlType),
}

#[async_trait]
impl FetchRemoteSubgraph for StudioClient {
    type Error = StudioFetchRemoteSdlError;
    async fn fetch_remote_subgraph(
        &self,
        graph_ref: GraphRef,
        subgraph_name: String,
    ) -> Result<RemoteSubgraph, Self::Error> {
        fetch::run(
            SubgraphFetchInput {
                graph_ref,
                subgraph_name: subgraph_name.clone(),
            },
            self,
        )
        .await
        .map_err(StudioFetchRemoteSdlError::from)
        .and_then(|result| {
            // We don't require a routing_url in config for this variant of a schema,
            // if one isn't provided, just use the routing URL from the graph registry (if it exists).
            if let rover_client::shared::SdlType::Subgraph {
                routing_url: Some(graph_registry_routing_url),
            } = result.sdl.r#type
            {
                Ok(RemoteSubgraph {
                    name: subgraph_name,
                    routing_url: graph_registry_routing_url,
                    schema: result.sdl.contents,
                })
            } else {
                Err(StudioFetchRemoteSdlError::InvalidSdlType(result.sdl.r#type))
            }
        })
    }
}

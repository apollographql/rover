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

#[derive(Clone, Debug, Eq, PartialEq, Builder, Getters)]
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
    type Error: std::error::Error + Send + Sync + 'static;
    async fn fetch_remote_subgraph(
        &self,
        graph_ref: GraphRef,
        subgraph_name: String,
    ) -> Result<RemoteSubgraph, Self::Error>;
}

#[derive(thiserror::Error, Debug)]
pub enum StudioFetchRemoteSdlError {
    #[error("Failed to build the client.\n{}", .0)]
    Reqwest(RoverError),
    #[error("Failed to fetch the subgraph from remote.\n{}", .0)]
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

#[cfg(test)]
mod test {

    use std::{str::FromStr, time::Duration};

    use anyhow::Result;
    use houston::Credential;
    use httpmock::MockServer;
    use rover_client::{blocking::StudioClient, shared::GraphRef};
    use rstest::{fixture, rstest};
    use serde_json::json;
    use speculoos::prelude::*;

    use crate::utils::effect::test::SUBGRAPH_FETCH_QUERY;

    use super::{FetchRemoteSubgraph, RemoteSubgraph};

    #[fixture]
    #[once]
    fn query() -> &'static str {
        SUBGRAPH_FETCH_QUERY
    }

    #[rstest]
    #[timeout(Duration::from_secs(1))]
    #[tokio::test]
    async fn test_studio_fetch_remote_subgraph_success(query: &str) -> Result<()> {
        let version = "test-version";
        let is_sudo = false;
        let server = MockServer::start();
        let reqwest_client = reqwest::Client::new();
        let server_address = server.address();
        let endpoint = format!(
            "http://{}:{}/graphql",
            server_address.ip(),
            server_address.port()
        );
        let studio_client = StudioClient::new(
            Credential {
                api_key: "test-api-key".to_string(),
                origin: houston::CredentialOrigin::EnvVar,
            },
            &endpoint,
            version,
            is_sudo,
            reqwest_client,
            None,
        );
        let _mock = server.mock(|when, then| {
            let expected_body = json!({
                "query": query,
                "variables": {
                    "graph_ref": "graph@variant",
                    "subgraph_name": "subgraph_name"
                },
                "operationName": "SubgraphFetchQuery"
            });
            when.path("/graphql")
                .method(httpmock::Method::POST)
                .json_body_obj(&expected_body);
            let result_body = json!({
                "data": {
                    "variant": {
                        "__typename": "GraphVariant",
                        "subgraph": {
                            "url": "http://example.com/graphql",
                            "activePartialSchema": {
                                "sdl": "def",
                            },
                            "subgraphs": [{
                                "name": "ghi"
                            }]
                        }
                    }
                }
            });
            then.json_body_obj(&result_body);
        });
        let graph_ref = GraphRef::from_str("graph@variant")?;
        let result = studio_client
            .fetch_remote_subgraph(graph_ref, "subgraph_name".to_string())
            .await;
        assert_that!(result).is_ok().is_equal_to(RemoteSubgraph {
            name: "subgraph_name".to_string(),
            routing_url: "http://example.com/graphql".to_string(),
            schema: "def".to_string(),
        });
        Ok(())
    }
}

use std::collections::HashMap;

use async_trait::async_trait;
use rover_client::{blocking::GraphQLClient, operations::subgraph::introspect, RoverClientError};
use url::Url;

use crate::{utils::client::StudioClientConfig, RoverError};

#[cfg_attr(test, derive(thiserror::Error, Debug))]
#[cfg_attr(test, error("{}", .0))]
#[cfg(test)]
pub struct MockIntrospectSubgraphError(String);

#[cfg_attr(test, mockall::automock(type Error = MockIntrospectSubgraphError;))]
#[async_trait]
pub trait IntrospectSubgraph {
    type Error: std::error::Error + 'static;
    async fn introspect_subgraph(
        &self,
        endpoint: Url,
        headers: HashMap<String, String>,
    ) -> Result<String, Self::Error>;
}

#[derive(thiserror::Error, Debug)]
pub enum RoverIntrospectSubgraphError {
    #[error("Failed to build the reuest client")]
    Build(RoverError),
    #[error("Failed to introspect the graphql endpoint")]
    IntrospectionError(#[from] RoverClientError),
}

#[async_trait]
impl IntrospectSubgraph for StudioClientConfig {
    type Error = RoverIntrospectSubgraphError;
    async fn introspect_subgraph(
        &self,
        endpoint: Url,
        headers: HashMap<String, String>,
    ) -> Result<String, Self::Error> {
        let client = self
            .get_reqwest_client()
            .map_err(RoverError::from)
            .map_err(RoverIntrospectSubgraphError::Build)?;
        let client = GraphQLClient::new(&endpoint.to_string(), client, self.retry_period);
        let response = introspect::run(
            introspect::SubgraphIntrospectInput { headers },
            &client,
            false,
        )
        .await?;
        Ok(response.result.to_string())
    }
}

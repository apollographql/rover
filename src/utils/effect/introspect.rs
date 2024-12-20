use std::collections::HashMap;

use async_trait::async_trait;
use rover_client::{operations::subgraph::introspect, RoverClientError};
use url::Url;

use crate::{utils::client::StudioClientConfig, RoverError};

#[cfg_attr(test, derive(thiserror::Error, Debug))]
#[cfg_attr(test, error("{}", .0))]
#[cfg(test)]
pub struct MockIntrospectSubgraphError(String);

#[cfg_attr(test, mockall::automock(type Error = MockIntrospectSubgraphError;))]
#[async_trait]
pub trait IntrospectSubgraph {
    type Error: std::error::Error + Send + Sync + 'static;
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
        let response = introspect::run(
            introspect::SubgraphIntrospectInput {
                headers,
                endpoint: endpoint.clone(),
                should_retry: false,
                retry_period: self.retry_period(),
            },
            &client,
        )
        .await?;
        Ok(response.result.to_string())
    }
}

#[cfg(test)]
mod test {
    use std::{collections::HashMap, str::FromStr, time::Duration};

    use anyhow::Result;
    use assert_fs::TempDir;
    use camino::Utf8PathBuf;
    use houston::Config;
    use httpmock::MockServer;
    use rstest::{fixture, rstest};
    use serde_json::json;
    use speculoos::prelude::*;

    use crate::utils::{
        client::{ClientBuilder, ClientTimeout, StudioClientConfig},
        effect::test::SUBGRAPH_INTROSPECTION_QUERY,
    };

    use super::IntrospectSubgraph;

    #[fixture]
    #[once]
    fn query() -> &'static str {
        SUBGRAPH_INTROSPECTION_QUERY
    }

    #[rstest]
    #[timeout(Duration::from_secs(1))]
    #[tokio::test]
    async fn test_introspect_subgraph_success(query: &str) -> Result<()> {
        let server = MockServer::start_async().await;
        server.mock(|when, then| {
            let expected_body = json!({
                "query": query,
                "variables": null,
                "operationName": "SubgraphIntrospectQuery"
            });
            when.path("/graphql")
                .header("x-test-name", "x-test-value")
                .method(httpmock::Method::POST)
                .json_body_obj(&expected_body);
            then.status(200).json_body(json!({
                "data": {
                    "_service": {
                        "sdl": "type Query { test: String! }"
                    }
                }
            }));
        });
        let server_address = server.address();
        let endpoint = format!(
            "http://{}:{}/graphql",
            server_address.ip(),
            server_address.port()
        );
        let endpoint = url::Url::from_str(&endpoint)?;
        let home = TempDir::new()?;
        let config = Config {
            home: Utf8PathBuf::from_path_buf(home.path().to_path_buf()).unwrap(),
            override_api_key: None,
        };
        let studio_client_config = StudioClientConfig::new(
            None,
            config,
            false,
            ClientBuilder::default(),
            ClientTimeout::default(),
        );
        let headers = HashMap::from_iter([("x-test-name".to_string(), "x-test-value".to_string())]);
        let result = studio_client_config
            .introspect_subgraph(endpoint, headers)
            .await;
        assert_that!(result)
            .is_ok()
            .is_equal_to("type Query { test: String! }".to_string());
        Ok(())
    }
}

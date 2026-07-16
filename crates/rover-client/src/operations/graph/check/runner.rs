use tower::{Service, ServiceExt};

use super::{service::GraphCheck, types::CheckSchemaAsyncInput};
use crate::{blocking::StudioClient, shared::CheckRequestSuccessResult, RoverClientError};

/// The main function to be used from this module.
/// This function takes a proposed schema and validates it against a published
/// schema.
pub async fn run(
    input: CheckSchemaAsyncInput,
    client: &StudioClient,
) -> Result<CheckRequestSuccessResult, RoverClientError> {
    let mut service = GraphCheck::new(
        client
            .studio_graphql_service()
            .map_err(|err| RoverClientError::ServiceReady(Box::new(err)))?,
    );
    let service = service.ready().await?;
    service.call(input).await
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use houston::{Credential, CredentialOrigin};
    use httpmock::prelude::*;
    use reqwest::Client as ReqwestClient;

    use super::*;
    use crate::shared::{CheckConfig, GitContext};

    /// A schema too large for Studio comes back as a 413. Through the tower stack
    /// this surfaces as a GraphQL deserialization failure carrying the status,
    /// which the check service translates into a clear `RequestTooLarge` error
    /// rather than an opaque parse failure. See #1383.
    #[tokio::test]
    async fn run_maps_413_to_request_too_large() {
        let server = MockServer::start_async().await;
        let mock = server.mock(|when, then| {
            when.method(POST).body_includes("GraphCheckMutation");
            then.status(413).body("payload too large");
        });

        let client = StudioClient::new(
            Credential {
                api_key: "test".to_string(),
                origin: CredentialOrigin::EnvVar,
                expires_at: None,
            },
            &server.url("/"),
            "test-version",
            false,
            ReqwestClient::new(),
            Duration::from_secs(1),
        );
        let input = CheckSchemaAsyncInput {
            graph_ref: "test-graph@test-variant".parse().unwrap(),
            proposed_schema: "type Query { hello: String }".to_string(),
            git_context: GitContext::default(),
            config: CheckConfig {
                query_count_threshold: None,
                query_count_threshold_percentage: None,
                validation_period: None,
            },
        };

        let result = run(input, &client).await;
        mock.assert();
        assert!(
            matches!(result, Err(RoverClientError::RequestTooLarge { .. })),
            "expected RequestTooLarge, got {result:?}"
        );
    }
}

use tower::{Service, ServiceExt};

use super::{
    service::{GraphFetch, GraphFetchRequest},
    types::GraphFetchInput,
};
use crate::{blocking::StudioClient, shared::FetchResponse, RoverClientError};

/// Fetch the SDL for a graph variant from Apollo Studio using a graph ref.
///
/// On success, the response contains the full SDL string for that variant.
///
/// This returns an error if the graph does not exist, if no schema has been published for the
/// requested variant, or if the Studio API call fails.
pub async fn run(
    input: GraphFetchInput,
    client: &StudioClient,
) -> Result<FetchResponse, RoverClientError> {
    let mut service = GraphFetch::new(
        client
            .studio_graphql_service()
            .map_err(|err| RoverClientError::ServiceReady(Box::new(err)))?,
    );
    let service = service.ready().await?;
    service
        .call(GraphFetchRequest::new(input))
        .await
        .map_err(|err| client.refine_rejected_credential_error(err))
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use houston::{Credential, CredentialOrigin};
    use httpmock::prelude::*;
    use reqwest::Client as ReqwestClient;
    use rover_studio::types::GraphRef;
    use rstest::{fixture, rstest};
    use serde_json::json;
    use speculoos::prelude::*;

    use super::*;
    use crate::operations::graph::fetch::service::{
        get_schema_from_response_data, graph_fetch_query,
    };

    #[fixture]
    fn graph_ref() -> GraphRef {
        GraphRef::new("mygraph", Some("current")).unwrap()
    }

    fn client_with(server: &MockServer, api_key: &str) -> StudioClient {
        StudioClient::new(
            Credential {
                api_key: api_key.to_string(),
                origin: CredentialOrigin::EnvVar,
                expires_at: None,
            },
            &server.url("/"),
            "test-version",
            false,
            ReqwestClient::new(),
            Duration::from_secs(1),
        )
    }

    /// A credential the registry rejects at the gateway (HTTP 200, `"data": null`, a
    /// body-level "Invalid credentials" error) is classified as
    /// `GraphQLServiceError::InvalidCredentials`, which on its own can only ever
    /// produce the weaker `RoverClientError::InvalidKey`. `run` refines that against
    /// the credential's own shape once it's back, upgrading to `MalformedKey` when
    /// the key sent could never have been valid in the first place.
    #[tokio::test]
    async fn run_upgrades_a_body_level_rejection_to_malformed_when_the_key_shape_is_wrong() {
        let server = MockServer::start_async().await;
        let mock = server.mock(|when, then| {
            when.method(POST).body_includes("GraphFetchQuery");
            then.status(200).json_body(json!({
                "data": null,
                "errors": [{ "message": "Unauthorized: Invalid credentials provided" }]
            }));
        });

        let client = client_with(&server, "not-a-real-key");
        let input = GraphFetchInput {
            graph_ref: GraphRef::new("mygraph", Some("current")).unwrap(),
        };

        let result = run(input, &client).await;
        mock.assert();
        assert!(
            matches!(result, Err(RoverClientError::MalformedKey)),
            "expected MalformedKey, got {result:?}"
        );
    }

    /// Same body-level rejection, but the credential sent is shaped like a real key
    /// (just revoked/expired/unknown) - the refine step should leave it as
    /// `InvalidKey` rather than upgrading it.
    #[tokio::test]
    async fn run_keeps_a_body_level_rejection_as_invalid_when_the_key_shape_is_fine() {
        let server = MockServer::start_async().await;
        let mock = server.mock(|when, then| {
            when.method(POST).body_includes("GraphFetchQuery");
            then.status(200).json_body(json!({
                "data": null,
                "errors": [{ "message": "Unauthorized: Invalid credentials provided" }]
            }));
        });

        let client = client_with(&server, "user:my-username:secretkey");
        let input = GraphFetchInput {
            graph_ref: GraphRef::new("mygraph", Some("current")).unwrap(),
        };

        let result = run(input, &client).await;
        mock.assert();
        assert!(
            matches!(result, Err(RoverClientError::InvalidKey)),
            "expected InvalidKey, got {result:?}"
        );
    }

    /// Verifies that a response containing a schema document returns the SDL string successfully.
    #[rstest]
    fn get_schema_from_response_data_works(graph_ref: GraphRef) {
        let json_response = json!({
            "frontendUrlRoot": "https://studio.apollographql.com",
            "graph": {
                "variant": {
                    "latestPublication": {
                       "schema": {
                            "document": "type Query { hello: String }"
                        }
                    }
                },
                "variants": []
            }
        });
        let data: graph_fetch_query::ResponseData = serde_json::from_value(json_response).unwrap();
        let output = get_schema_from_response_data(data, graph_ref);
        assert!(output.is_ok());
        assert_eq!(output.unwrap(), "type Query { hello: String }".to_string());
    }

    /// Verifies that a null graph in the response produces a GraphNotFound error.
    #[rstest]
    fn get_schema_from_response_data_errs_on_no_service(graph_ref: GraphRef) {
        let json_response =
            json!({ "service": null, "frontendUrlRoot": "https://studio.apollographql.com" });
        let data: graph_fetch_query::ResponseData = serde_json::from_value(json_response).unwrap();
        assert_that!(get_schema_from_response_data(data, graph_ref))
            .is_err()
            .matches(|err| matches!(err, crate::RoverClientError::GraphNotFound { .. }));
    }

    /// Verifies that a response with a null variant (no published schema) produces a
    /// NoSchemaForVariant error.
    #[rstest]
    fn get_schema_from_response_data_errs_on_no_schema(graph_ref: GraphRef) {
        let json_response = json!({
            "frontendUrlRoot": "https://studio.apollographql.com/",
            "graph": {
                "schema": null,
                "variants": [],
            },
        });
        let data: graph_fetch_query::ResponseData = serde_json::from_value(json_response).unwrap();
        assert_that!(get_schema_from_response_data(data, graph_ref))
            .is_err()
            .matches(|err| matches!(err, crate::RoverClientError::NoSchemaForVariant { .. }));
    }
}

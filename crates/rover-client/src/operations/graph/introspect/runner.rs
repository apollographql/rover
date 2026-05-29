use std::convert::TryFrom;

use http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
use reqwest::Client;
use rover_graphql::{GraphQLLayer, GraphQLServiceError};
use rover_http::{extend_headers::ExtendHeadersLayer, retry::RetryPolicy, ReqwestService};
use tower::{retry::RetryLayer, ServiceBuilder, ServiceExt};

use crate::{
    error::RoverClientError,
    operations::graph::introspect::{
        service::{
            graph_introspect_query, GraphIntrospect, GraphIntrospectError, GraphIntrospectLegacy,
        },
        types::*,
        Schema,
    },
};

/// The main function to be used from this module. This function fetches a
/// schema from a GraphQL endpoint and returns it as an SDL.
pub async fn run(
    input: GraphIntrospectInput,
    client: &Client,
) -> Result<GraphIntrospectResponse, RoverClientError> {
    let http_service = ReqwestService::builder()
        .client(client.clone())
        .build()
        .map_err(|err| RoverClientError::ServiceReady(Box::new(err)))?;

    let mut header_map = HeaderMap::new();
    for (header_key, header_value) in &input.headers {
        header_map.insert(
            HeaderName::from_bytes(header_key.as_bytes())?,
            HeaderValue::from_str(header_value)?,
        );
    }

    let response_data = if input.use_legacy_introspection_query {
        let legacy = ServiceBuilder::new()
            .layer_fn(GraphIntrospectLegacy::new)
            .layer(GraphQLLayer::new(input.endpoint.clone()))
            .option_layer(retry_layer(&input))
            .layer(ExtendHeadersLayer::new(header_map.clone()))
            .service(http_service.clone());
        legacy.oneshot(()).await?
    } else {
        let modern = ServiceBuilder::new()
            .layer_fn(GraphIntrospect::new)
            .layer(GraphQLLayer::new(input.endpoint.clone()))
            .option_layer(retry_layer(&input))
            .layer(ExtendHeadersLayer::new(header_map.clone()))
            .service(http_service.clone());
        match modern.oneshot(()).await {
            Ok(data) => data,
            Err(GraphIntrospectError::ModernGraphQL(err)) if should_fall_back_to_legacy(&err) => {
                tracing::debug!(
                    "modern introspection query rejected ({err}); falling back to legacy query"
                );
                let legacy = ServiceBuilder::new()
                    .layer_fn(GraphIntrospectLegacy::new)
                    .layer(GraphQLLayer::new(input.endpoint.clone()))
                    .option_layer(retry_layer(&input))
                    .layer(ExtendHeadersLayer::new(header_map.clone()))
                    .service(http_service.clone());
                legacy.oneshot(()).await?
            }
            Err(other) => return Err(other.into()),
        }
    };

    build_response(response_data)
}

fn retry_layer(input: &GraphIntrospectInput) -> Option<RetryLayer<RetryPolicy>> {
    if input.should_retry {
        Some(RetryLayer::new(RetryPolicy::new(input.retry_period)))
    } else {
        None
    }
}

fn build_response(
    response: graph_introspect_query::ResponseData,
) -> Result<GraphIntrospectResponse, RoverClientError> {
    match Schema::try_from(response) {
        Ok(schema) => Ok(GraphIntrospectResponse {
            schema_sdl: schema.encode(),
        }),
        Err(msg) => Err(RoverClientError::IntrospectionError { msg: msg.into() }),
    }
}

/// Decide whether a failed modern-introspection attempt looks like the server
/// doesn't implement the October-2021 spec additions.
///
/// Matches:
/// - GraphQL body errors whose joined message mentions any of `__InputValue`,
///   `includeDeprecated`, `isDeprecated`, or `deprecationReason` — what Apollo
///   Server and graphql-js emit when meta-schema validation rejects the
///   unknown fields.
/// - HTTP 422 Unprocessable Entity — the server understood the request but
///   rejected its contents. Surfaces as `Deserialization` because the response
///   body is typically not a valid GraphQL response envelope.
fn should_fall_back_to_legacy<T>(err: &GraphQLServiceError<T>) -> bool
where
    T: Send + Sync + std::fmt::Debug,
{
    const MARKERS: &[&str] = &[
        "__InputValue",
        "includeDeprecated",
        "isDeprecated",
        "deprecationReason",
    ];
    match err {
        GraphQLServiceError::NoData(errors) | GraphQLServiceError::PartialError { errors, .. } => {
            errors
                .iter()
                .any(|e| MARKERS.iter().any(|m| e.message.contains(m)))
        }
        GraphQLServiceError::Deserialization { status_code, .. } => {
            *status_code == StatusCode::UNPROCESSABLE_ENTITY
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use bytes::Bytes;
    use graphql_client::Error as GraphQLError;
    use httpmock::prelude::*;
    use serde_json::json;

    use super::*;

    fn graphql_error(message: &str) -> GraphQLError {
        GraphQLError {
            message: message.to_string(),
            locations: None,
            path: None,
            extensions: None,
        }
    }

    fn deserialization_error(status: StatusCode) -> GraphQLServiceError<()> {
        let json_err = serde_json::from_str::<serde_json::Value>("not json").unwrap_err();
        GraphQLServiceError::Deserialization {
            error: json_err,
            data: Bytes::new(),
            status_code: status,
        }
    }

    #[test]
    fn fallback_matches_input_value_meta_schema_error() {
        let err: GraphQLServiceError<()> = GraphQLServiceError::NoData(vec![graphql_error(
            "Cannot query field \"isDeprecated\" on type \"__InputValue\"",
        )]);
        assert!(should_fall_back_to_legacy(&err));
    }

    #[test]
    fn fallback_matches_each_marker_individually() {
        for marker in [
            "__InputValue",
            "includeDeprecated",
            "isDeprecated",
            "deprecationReason",
        ] {
            let err: GraphQLServiceError<()> = GraphQLServiceError::NoData(vec![graphql_error(
                &format!("server complained about {marker}"),
            )]);
            assert!(
                should_fall_back_to_legacy(&err),
                "expected fallback for marker {marker}"
            );
        }
    }

    #[test]
    fn fallback_ignores_unrelated_graphql_errors() {
        let err: GraphQLServiceError<()> =
            GraphQLServiceError::NoData(vec![graphql_error("permission denied")]);
        assert!(!should_fall_back_to_legacy(&err));
    }

    #[test]
    fn fallback_matches_partial_error_with_marker() {
        let err: GraphQLServiceError<()> = GraphQLServiceError::PartialError {
            data: (),
            errors: vec![graphql_error(
                "Cannot query field \"includeDeprecated\" on type \"__Field\"",
            )],
            friendly_errors_detail: Vec::new(),
        };
        assert!(should_fall_back_to_legacy(&err));
    }

    #[test]
    fn fallback_matches_422_deserialization_error() {
        let err = deserialization_error(StatusCode::UNPROCESSABLE_ENTITY);
        assert!(should_fall_back_to_legacy(&err));
    }

    #[test]
    fn fallback_ignores_500_deserialization_error() {
        let err = deserialization_error(StatusCode::INTERNAL_SERVER_ERROR);
        assert!(!should_fall_back_to_legacy(&err));
    }

    fn introspect_input(endpoint: url::Url) -> GraphIntrospectInput {
        GraphIntrospectInput {
            headers: Default::default(),
            endpoint,
            should_retry: false,
            retry_period: Duration::from_secs(1),
            use_legacy_introspection_query: false,
        }
    }

    fn empty_legacy_introspection_body() -> serde_json::Value {
        json!({
            "data": {
                "__schema": {
                    "queryType": { "name": "Query" },
                    "mutationType": null,
                    "subscriptionType": null,
                    "types": [],
                    "directives": []
                }
            }
        })
    }

    #[tokio::test]
    async fn run_falls_back_to_legacy_on_input_value_error() {
        let server = MockServer::start_async().await;
        let path = "/graphql";

        let modern_mock = server
            .mock_async(|when, then| {
                when.method(POST)
                    .path(path)
                    .body_includes("GraphIntrospectQuery");
                then.status(200).json_body(json!({
                    "errors": [{
                        "message": "Cannot query field \"isDeprecated\" on type \"__InputValue\""
                    }]
                }));
            })
            .await;

        let legacy_mock = server
            .mock_async(|when, then| {
                when.method(POST)
                    .path(path)
                    .body_includes("GraphIntrospectLegacyQuery");
                then.status(200)
                    .json_body(empty_legacy_introspection_body());
            })
            .await;

        let endpoint = url::Url::parse(&server.url(path)).unwrap();
        let result = run(introspect_input(endpoint), &Client::new()).await;

        assert_eq!(modern_mock.calls_async().await, 1);
        assert_eq!(legacy_mock.calls_async().await, 1);
        assert!(
            result.is_ok(),
            "expected fallback to succeed, got {result:?}"
        );
    }

    #[tokio::test]
    async fn run_falls_back_to_legacy_on_422() {
        let server = MockServer::start_async().await;
        let path = "/graphql";

        let modern_mock = server
            .mock_async(|when, then| {
                when.method(POST)
                    .path(path)
                    .body_includes("GraphIntrospectQuery");
                then.status(422).body("unprocessable");
            })
            .await;

        let legacy_mock = server
            .mock_async(|when, then| {
                when.method(POST)
                    .path(path)
                    .body_includes("GraphIntrospectLegacyQuery");
                then.status(200)
                    .json_body(empty_legacy_introspection_body());
            })
            .await;

        let endpoint = url::Url::parse(&server.url(path)).unwrap();
        let result = run(introspect_input(endpoint), &Client::new()).await;

        assert_eq!(modern_mock.calls_async().await, 1);
        assert_eq!(legacy_mock.calls_async().await, 1);
        assert!(
            result.is_ok(),
            "expected fallback to succeed, got {result:?}"
        );
    }

    #[tokio::test]
    async fn run_uses_legacy_directly_when_override_set() {
        let server = MockServer::start_async().await;
        let path = "/graphql";

        let modern_mock = server
            .mock_async(|when, then| {
                when.method(POST)
                    .path(path)
                    .body_includes("GraphIntrospectQuery");
                then.status(500).body("");
            })
            .await;

        let legacy_mock = server
            .mock_async(|when, then| {
                when.method(POST)
                    .path(path)
                    .body_includes("GraphIntrospectLegacyQuery");
                then.status(200)
                    .json_body(empty_legacy_introspection_body());
            })
            .await;

        let endpoint = url::Url::parse(&server.url(path)).unwrap();
        let mut input = introspect_input(endpoint);
        input.use_legacy_introspection_query = true;
        let result = run(input, &Client::new()).await;

        assert_eq!(modern_mock.calls_async().await, 0);
        assert_eq!(legacy_mock.calls_async().await, 1);
        assert!(
            result.is_ok(),
            "expected direct legacy call to succeed, got {result:?}"
        );
    }
}

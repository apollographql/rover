mod service;

use std::time::Duration;

use graphql_client::GraphQLQuery;
use rover_studio::types::GraphRef;
use rover_tower::poll_retry::PollRetryPolicy;
pub use service::{ContractPreviewResult, ContractPreviewStart};
use tower::{Service, ServiceBuilder, ServiceExt};

pub use crate::shared::{AsyncBuildStatus, ContractFilterConfig, PreviewJobResponse};
use crate::{blocking::StudioClient, RoverClientError};

#[derive(GraphQLQuery)]
#[graphql(
    query_path = "src/operations/contract/preview/preview_async_mutation.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "Eq, PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
pub(crate) struct ContractPreviewAsyncMutation;

#[derive(GraphQLQuery)]
#[graphql(
    query_path = "src/operations/contract/preview/contract_preview_result_query.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "Eq, PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
pub(crate) struct ContractPreviewResultQuery;

#[derive(GraphQLQuery)]
#[graphql(
    query_path = "src/operations/contract/preview/contract_preview_status_query.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "Eq, PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
pub(crate) struct ContractPreviewStatusQuery;

#[derive(Clone, Eq, PartialEq, Debug)]
pub struct ContractPreviewInput {
    pub graph_ref: GraphRef,
    pub filter_config: ContractFilterConfig,
}

impl From<ContractPreviewInput> for contract_preview_async_mutation::Variables {
    fn from(input: ContractPreviewInput) -> Self {
        let (graph_id, variant) = input.graph_ref.into_parts();
        Self {
            graph_id,
            variant,
            filters: contract_preview_async_mutation::FilterConfigInput {
                include: input.filter_config.include,
                exclude: input.filter_config.exclude,
                hide_unreachable_types: input.filter_config.hide_unreachable_types,
            },
        }
    }
}

/// Input to poll (or fetch the full result of) a build started by
/// `contractPreviewAsync`. `contractPreviewStatus` is a field on
/// `GraphVariant`, so checking status needs the same `graph_ref` used to
/// start the build.
#[derive(Clone, Eq, PartialEq, Debug)]
pub struct ContractPreviewStatusInput {
    pub graph_ref: GraphRef,
    pub build_id: String,
}

impl From<ContractPreviewStatusInput> for contract_preview_result_query::Variables {
    fn from(input: ContractPreviewStatusInput) -> Self {
        let (graph_id, variant) = input.graph_ref.into_parts();
        Self {
            graph_id,
            variant,
            build_id: input.build_id,
        }
    }
}

impl From<ContractPreviewStatusInput> for contract_preview_status_query::Variables {
    fn from(input: ContractPreviewStatusInput) -> Self {
        let (graph_id, variant) = input.graph_ref.into_parts();
        Self {
            graph_id,
            variant,
            build_id: input.build_id,
        }
    }
}

/// Builds the default `Service` for starting an async contract preview job,
/// layered over `client`'s Studio GraphQL service.
pub fn contract_preview_start_service(
    client: &StudioClient,
) -> Result<
    impl Service<ContractPreviewInput, Response = PreviewJobResponse, Error = RoverClientError> + Clone,
    RoverClientError,
> {
    Ok(ContractPreviewStart::new(
        client
            .studio_graphql_service()
            .map_err(|err| RoverClientError::ServiceReady(Box::new(err)))?,
    ))
}

/// Builds the default `Service` for fetching the full result of a contract
/// preview build, layered over `client`'s Studio GraphQL service.
pub fn contract_preview_result_service(
    client: &StudioClient,
) -> Result<
    impl Service<ContractPreviewStatusInput, Response = PreviewJobResponse, Error = RoverClientError>
        + Clone,
    RoverClientError,
> {
    Ok(ContractPreviewResult::new(
        client
            .studio_graphql_service()
            .map_err(|err| RoverClientError::ServiceReady(Box::new(err)))?,
    ))
}

/// Start an async contract preview job using an already-composed `Service`,
/// returning its (pending) status immediately.
pub async fn start<S>(
    input: ContractPreviewInput,
    mut service: S,
) -> Result<PreviewJobResponse, RoverClientError>
where
    S: Service<ContractPreviewInput, Response = PreviewJobResponse, Error = RoverClientError>,
{
    let service = service.ready().await?;
    service.call(input).await
}

/// Fetch the full result of a previously started contract preview build
/// using an already-composed `Service`.
pub async fn result<S>(
    input: ContractPreviewStatusInput,
    mut service: S,
) -> Result<PreviewJobResponse, RoverClientError>
where
    S: Service<ContractPreviewStatusInput, Response = PreviewJobResponse, Error = RoverClientError>,
{
    let service = service.ready().await?;
    service.call(input).await
}

/// Start an async contract preview build then poll until it reaches a
/// terminal state.
pub async fn run(
    input: ContractPreviewInput,
    client: &StudioClient,
    checks_timeout_seconds: u64,
) -> Result<PreviewJobResponse, RoverClientError> {
    let graph_ref = input.graph_ref.clone();
    let start_service = contract_preview_start_service(client)?;
    let started = start(input, start_service).await?;
    let status_input = ContractPreviewStatusInput {
        graph_ref,
        build_id: started.build_id,
    };
    poll(status_input, client, checks_timeout_seconds).await
}

/// Continuously poll the status of an already-started contract preview build,
/// then fetch its full result once it's finished.
pub async fn poll(
    status_input: ContractPreviewStatusInput,
    client: &StudioClient,
    checks_timeout_seconds: u64,
) -> Result<PreviewJobResponse, RoverClientError> {
    let build_id = status_input.build_id.clone();
    let mut status_service = ServiceBuilder::new()
        .retry(PollRetryPolicy::new(
            Duration::from_secs(5),
            Duration::from_secs(checks_timeout_seconds),
            {
                let build_id = build_id.clone();
                move || RoverClientError::PreviewTimeoutError {
                    build_id: build_id.clone(),
                }
            },
        ))
        .service(service::ContractPreviewStatus::new(
            client
                .studio_graphql_service()
                .map_err(|err| RoverClientError::ServiceReady(Box::new(err)))?,
        ));
    status_service
        .ready()
        .await?
        .call(status_input.clone())
        .await?;

    let result_service = contract_preview_result_service(client)?;
    result(status_input, result_service)
        .await
        .map_err(|source| RoverClientError::PreviewResultUnavailable {
            build_id,
            source: Box::new(source),
        })
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use houston::{Credential, CredentialOrigin};
    use httpmock::prelude::*;
    use reqwest::Client as ReqwestClient;
    use serde_json::json;

    use super::*;
    use crate::shared::{AsyncBuildStatus, ContractFilterConfig};

    fn test_client(server_url: &str) -> StudioClient {
        StudioClient::new(
            Credential {
                api_key: "test".to_string(),
                origin: CredentialOrigin::EnvVar,
                expires_at: None,
            },
            server_url,
            "test-version",
            false,
            ReqwestClient::new(),
            Duration::from_secs(1),
        )
    }

    fn test_input() -> ContractPreviewInput {
        ContractPreviewInput {
            graph_ref: "test-graph@test-variant".parse().unwrap(),
            filter_config: ContractFilterConfig {
                include: vec!["public".to_string()],
                exclude: Vec::new(),
                hide_unreachable_types: false,
            },
        }
    }

    #[tokio::test]
    async fn start_returns_a_pending_response_carrying_the_build_id() {
        let server = MockServer::start_async().await;
        server.mock(|when, then| {
            when.method(POST)
                .body_includes("ContractPreviewAsyncMutation");
            then.status(200).json_body(json!({
                "data": { "graph": { "variant": {
                    "contractPreviewAsync": { "buildID": "build-123" }
                } } }
            }));
        });

        let client = test_client(&server.url("/"));
        let response = start(
            test_input(),
            contract_preview_start_service(&client).unwrap(),
        )
        .await
        .unwrap();

        assert_eq!(response.build_id, "build-123");
        assert_eq!(response.status, AsyncBuildStatus::Pending);
        assert_eq!(response.graph_ref.to_string(), "test-graph@test-variant");
    }

    #[tokio::test]
    async fn run_polls_status_then_fetches_full_result_once_finished() {
        let server = MockServer::start_async().await;
        server.mock(|when, then| {
            when.method(POST)
                .body_includes("ContractPreviewAsyncMutation");
            then.status(200).json_body(json!({
                "data": { "graph": { "variant": {
                    "contractPreviewAsync": { "buildID": "build-123" }
                } } }
            }));
        });
        server.mock(|when, then| {
            when.method(POST)
                .body_includes("ContractPreviewStatusQuery");
            then.status(200).json_body(json!({
                "data": { "graph": { "variant": {
                    "contractPreviewStatus": { "__typename": "ContractPreviewSuccess" }
                } } }
            }));
        });
        server.mock(|when, then| {
            when.method(POST)
                .body_includes("ContractPreviewResultQuery");
            then.status(200).json_body(json!({
                "data": { "graph": { "variant": {
                    "contractPreviewStatus": {
                        "__typename": "ContractPreviewSuccess",
                        "apiDocument": "type Query { hi: String }",
                        "coreDocument": "supergraph"
                    }
                } } }
            }));
        });

        let response = run(test_input(), &test_client(&server.url("/")), 30)
            .await
            .unwrap();

        assert_eq!(response.status, AsyncBuildStatus::Success);
        assert_eq!(response.build_id, "build-123");
        assert_eq!(
            response.api_schema,
            Some("type Query { hi: String }".to_string())
        );
    }

    #[tokio::test]
    async fn run_fails_fast_when_the_started_build_is_not_pollable() {
        // Unlike a check workflow, a contract preview build has no
        // eventual-consistency lag: once `contractPreviewAsync` returns a
        // build ID, `contractPreviewStatus` for it should never come back
        // empty. If it does, that's a real error and must not be retried
        // until the poll times out.
        let server = MockServer::start_async().await;
        server.mock(|when, then| {
            when.method(POST)
                .body_includes("ContractPreviewAsyncMutation");
            then.status(200).json_body(json!({
                "data": { "graph": { "variant": {
                    "contractPreviewAsync": { "buildID": "build-123" }
                } } }
            }));
        });
        server.mock(|when, then| {
            when.method(POST)
                .body_includes("ContractPreviewStatusQuery");
            then.status(200).json_body(json!({
                "data": { "graph": { "variant": {
                    "contractPreviewStatus": null
                } } }
            }));
        });

        let err = run(test_input(), &test_client(&server.url("/")), 30)
            .await
            .unwrap_err();

        assert!(
            matches!(err, RoverClientError::AdhocError { .. }),
            "expected an immediate AdhocError, got {err:?}"
        );
    }
}

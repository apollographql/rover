mod service;

use std::time::Duration;

use graphql_client::GraphQLQuery;
use rover_studio::types::GraphRef;
use rover_tower::poll_retry::PollRetryPolicy;
pub use service::{ComposeAndFilterPreviewResult, ComposeAndFilterPreviewStart};
use tower::{Service, ServiceBuilder, ServiceExt};

pub use crate::shared::{AsyncBuildStatus, ContractFilterConfig, PreviewJobResponse};
use crate::{blocking::StudioClient, RoverClientError};

#[derive(GraphQLQuery)]
#[graphql(
    query_path = "src/operations/subgraph/preview/compose_and_filter_preview_async_mutation.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "Eq, PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
/// This struct is used to generate the module containing `Variables` and
/// `ResponseData` structs. Snake case of this name is the mod name, i.e.
/// compose_and_filter_preview_async_mutation.
pub(crate) struct ComposeAndFilterPreviewAsyncMutation;

#[derive(GraphQLQuery)]
#[graphql(
    query_path = "src/operations/subgraph/preview/compose_and_filter_preview_result_query.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "Eq, PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
pub(crate) struct ComposeAndFilterPreviewResultQuery;

#[derive(GraphQLQuery)]
#[graphql(
    query_path = "src/operations/subgraph/preview/compose_and_filter_preview_status_query.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "Eq, PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
pub(crate) struct ComposeAndFilterPreviewStatusQuery;

#[derive(Clone, Eq, PartialEq, Debug)]
pub struct SubgraphChange {
    pub name: String,
    /// `None` indicates that this subgraph should be removed prior to
    /// composition.
    pub info: Option<SubgraphChangeInfo>,
}

/// The subgraph changes to compose.
#[derive(Clone, Eq, PartialEq, Debug)]
pub struct SubgraphChangeInfo {
    pub routing_url: Option<String>,
    pub schema_document: Option<String>,
}

#[derive(Clone, Eq, PartialEq, Debug)]
pub struct ComposeAndFilterPreviewInput {
    pub graph_ref: GraphRef,
    /// `None` skips filtering (compose-only preview).
    pub filter_config: Option<ContractFilterConfig>,
    /// Hypothetical per-subgraph schema/routing-url changes or removals to
    /// apply before composing. Empty means "compose the variant's subgraphs
    /// as they currently are".
    pub subgraph_changes: Vec<SubgraphChange>,
}

/// Input to query status or results of a previous
/// `composeAndFilterPreviewAsync`.
#[derive(Clone, Eq, PartialEq, Debug)]
pub struct ComposeAndFilterPreviewStatusInput {
    pub graph_ref: GraphRef,
    pub build_id: String,
}

impl From<ComposeAndFilterPreviewInput> for compose_and_filter_preview_async_mutation::Variables {
    fn from(input: ComposeAndFilterPreviewInput) -> Self {
        let (graph_id, variant) = input.graph_ref.into_parts();
        Self {
            graph_id,
            variant,
            filter_config: input.filter_config.map(|filter_config| {
                compose_and_filter_preview_async_mutation::FilterConfigInput {
                    include: filter_config.include,
                    exclude: filter_config.exclude,
                    hide_unreachable_types: filter_config.hide_unreachable_types,
                }
            }),
            subgraph_changes: if input.subgraph_changes.is_empty() {
                None
            } else {
                Some(
                    input
                        .subgraph_changes
                        .into_iter()
                        .map(|change| {
                            compose_and_filter_preview_async_mutation::ComposeAndFilterPreviewSubgraphChange {
                                name: change.name,
                                info: change.info.map(|info| {
                                    compose_and_filter_preview_async_mutation::ComposeAndFilterPreviewSubgraphChangeInfo {
                                        routing_url: info.routing_url,
                                        schema_document: info.schema_document,
                                    }
                                }),
                            }
                        })
                        .collect(),
                )
            },
        }
    }
}

impl From<ComposeAndFilterPreviewStatusInput>
    for compose_and_filter_preview_result_query::Variables
{
    fn from(input: ComposeAndFilterPreviewStatusInput) -> Self {
        let (graph_id, variant) = input.graph_ref.into_parts();
        Self {
            graph_id,
            variant,
            build_id: input.build_id,
        }
    }
}

impl From<ComposeAndFilterPreviewStatusInput>
    for compose_and_filter_preview_status_query::Variables
{
    fn from(input: ComposeAndFilterPreviewStatusInput) -> Self {
        let (graph_id, variant) = input.graph_ref.into_parts();
        Self {
            graph_id,
            variant,
            build_id: input.build_id,
        }
    }
}

/// Builds the default `Service` for starting an async compose-and-filter
/// preview job, layered over `client`'s Studio GraphQL service.
pub fn compose_and_filter_preview_start_service(
    client: &StudioClient,
) -> Result<
    impl Service<
            ComposeAndFilterPreviewInput,
            Response = PreviewJobResponse,
            Error = RoverClientError,
        > + Clone,
    RoverClientError,
> {
    Ok(ComposeAndFilterPreviewStart::new(
        client
            .studio_graphql_service()
            .map_err(|err| RoverClientError::ServiceReady(Box::new(err)))?,
    ))
}

/// Builds the default `Service` for fetching the full result of a
/// compose-and-filter preview build, layered over `client`'s Studio GraphQL
/// service.
pub fn compose_and_filter_preview_result_service(
    client: &StudioClient,
) -> Result<
    impl Service<
            ComposeAndFilterPreviewStatusInput,
            Response = PreviewJobResponse,
            Error = RoverClientError,
        > + Clone,
    RoverClientError,
> {
    Ok(ComposeAndFilterPreviewResult::new(
        client
            .studio_graphql_service()
            .map_err(|err| RoverClientError::ServiceReady(Box::new(err)))?,
    ))
}

/// Start an async compose-and-filter preview job using an already-composed
/// `Service`, returning its (pending) status immediately.
pub async fn start<S>(
    input: ComposeAndFilterPreviewInput,
    mut service: S,
) -> Result<PreviewJobResponse, RoverClientError>
where
    S: Service<
        ComposeAndFilterPreviewInput,
        Response = PreviewJobResponse,
        Error = RoverClientError,
    >,
{
    let service = service.ready().await?;
    service.call(input).await
}

/// Fetch the full result of a previously started compose-and-filter preview
/// build using an already-composed `Service`.
pub async fn result<S>(
    input: ComposeAndFilterPreviewStatusInput,
    mut service: S,
) -> Result<PreviewJobResponse, RoverClientError>
where
    S: Service<
        ComposeAndFilterPreviewStatusInput,
        Response = PreviewJobResponse,
        Error = RoverClientError,
    >,
{
    let service = service.ready().await?;
    service.call(input).await
}

/// Start an async compose-and-filter preview build then poll until it reaches
/// a terminal state.
pub async fn run(
    input: ComposeAndFilterPreviewInput,
    client: &StudioClient,
    checks_timeout_seconds: u64,
) -> Result<PreviewJobResponse, RoverClientError> {
    let graph_ref = input.graph_ref.clone();
    let start_service = compose_and_filter_preview_start_service(client)?;
    let started = start(input, start_service).await?;
    let status_input = ComposeAndFilterPreviewStatusInput {
        graph_ref,
        build_id: started.build_id,
    };
    poll(status_input, client, checks_timeout_seconds).await
}

/// Continuously poll the status of an already-started compose-and-filter
/// preview build, then fetch its full result once it's finished.
pub async fn poll(
    status_input: ComposeAndFilterPreviewStatusInput,
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
        .service(service::ComposeAndFilterPreviewStatus::new(
            client
                .studio_graphql_service()
                .map_err(|err| RoverClientError::ServiceReady(Box::new(err)))?,
        ));
    status_service
        .ready()
        .await?
        .call(status_input.clone())
        .await?;

    let result_service = compose_and_filter_preview_result_service(client)?;
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
    use crate::shared::AsyncBuildStatus;

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

    fn test_input() -> ComposeAndFilterPreviewInput {
        ComposeAndFilterPreviewInput {
            graph_ref: "test-graph@test-variant".parse().unwrap(),
            filter_config: None,
            subgraph_changes: Vec::new(),
        }
    }

    #[tokio::test]
    async fn start_returns_a_pending_response_carrying_the_build_id() {
        let server = MockServer::start_async().await;
        server.mock(|when, then| {
            when.method(POST)
                .body_includes("ComposeAndFilterPreviewAsyncMutation");
            then.status(200).json_body(json!({
                "data": { "graph": { "variant": {
                    "composeAndFilterPreviewAsync": { "buildID": "build-123" }
                } } }
            }));
        });

        let client = test_client(&server.url("/"));
        let response = start(
            test_input(),
            compose_and_filter_preview_start_service(&client).unwrap(),
        )
        .await
        .unwrap();

        assert_eq!(
            response,
            PreviewJobResponse {
                graph_ref: "test-graph@test-variant".parse().unwrap(),
                build_id: "build-123".to_string(),
                status: AsyncBuildStatus::Pending,
                api_schema: None,
                supergraph_schema: None,
                errors: Vec::new(),
            }
        );
    }

    #[tokio::test]
    async fn run_polls_status_then_fetches_full_result_once_finished() {
        let server = MockServer::start_async().await;
        server.mock(|when, then| {
            when.method(POST)
                .body_includes("ComposeAndFilterPreviewAsyncMutation");
            then.status(200).json_body(json!({
                "data": { "graph": { "variant": {
                    "composeAndFilterPreviewAsync": { "buildID": "build-123" }
                } } }
            }));
        });
        server.mock(|when, then| {
            when.method(POST)
                .body_includes("ComposeAndFilterPreviewStatusQuery");
            then.status(200).json_body(json!({
                "data": { "graph": { "variant": {
                    "composeAndFilterPreviewStatus": { "__typename": "ComposeAndFilterPreviewSuccess" }
                } } }
            }));
        });
        server.mock(|when, then| {
            when.method(POST)
                .body_includes("ComposeAndFilterPreviewResultQuery");
            then.status(200).json_body(json!({
                "data": { "graph": { "variant": {
                    "composeAndFilterPreviewStatus": {
                        "__typename": "ComposeAndFilterPreviewSuccess",
                        "composeResults": {
                            "apiSchemaDocument": "type Query { hi: String }",
                            "supergraphSchemaDocument": "supergraph"
                        },
                        "filterResults": null
                    }
                } } }
            }));
        });

        let response = run(test_input(), &test_client(&server.url("/")), 30)
            .await
            .unwrap();

        assert_eq!(
            response,
            PreviewJobResponse {
                graph_ref: "test-graph@test-variant".parse().unwrap(),
                build_id: "build-123".to_string(),
                status: AsyncBuildStatus::Success,
                api_schema: Some("type Query { hi: String }".to_string()),
                supergraph_schema: Some("supergraph".to_string()),
                errors: Vec::new(),
            }
        );
    }

    #[tokio::test]
    async fn run_fails_fast_when_the_started_build_is_not_pollable() {
        // Unlike a check workflow, a compose-and-filter preview build has no
        // eventual-consistency lag: once `composeAndFilterPreviewAsync` returns
        // a build ID, `composeAndFilterPreviewStatus` for it should never come
        // back empty. If it does, that's a real error and must not be retried
        // until the poll times out.
        let server = MockServer::start_async().await;
        server.mock(|when, then| {
            when.method(POST)
                .body_includes("ComposeAndFilterPreviewAsyncMutation");
            then.status(200).json_body(json!({
                "data": { "graph": { "variant": {
                    "composeAndFilterPreviewAsync": { "buildID": "build-123" }
                } } }
            }));
        });
        server.mock(|when, then| {
            when.method(POST)
                .body_includes("ComposeAndFilterPreviewStatusQuery");
            then.status(200).json_body(json!({
                "data": { "graph": { "variant": {
                    "composeAndFilterPreviewStatus": null
                } } }
            }));
        });

        let err = run(test_input(), &test_client(&server.url("/")), 30)
            .await
            .unwrap_err();

        let RoverClientError::AdhocError { msg } = err else {
            panic!("expected RoverClientError::AdhocError, got {err:?}");
        };
        assert_eq!(
            msg,
            "No compose-and-filter preview build found with ID build-123."
        );
    }
}

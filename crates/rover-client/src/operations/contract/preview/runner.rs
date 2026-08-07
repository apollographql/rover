use graphql_client::*;

use super::types::{ContractPreviewInput, ContractPreviewStatusInput};
use crate::{
    blocking::StudioClient,
    shared::{
        preview_poll::{poll_preview_build, require_variant},
        AsyncBuildStatus, PreviewJobResponse, PreviewKind,
    },
    RoverClientError,
};

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

/// Start an async contract preview job, returning its (pending) status
/// immediately.
pub async fn start(
    input: ContractPreviewInput,
    client: &StudioClient,
) -> Result<PreviewJobResponse, RoverClientError> {
    let graph_ref = input.graph_ref.clone();
    let response_data = client
        .post::<ContractPreviewAsyncMutation>(input.into())
        .await?;
    let build_id = require_variant(response_data.graph.map(|graph| graph.variant), &graph_ref)?
        .contract_preview_async
        .build_id;
    Ok(PreviewJobResponse {
        graph_ref,
        kind: PreviewKind::Contract,
        build_id,
        status: AsyncBuildStatus::Pending,
        api_schema: None,
        supergraph_schema: None,
        errors: Vec::new(),
    })
}

/// Check the status (without fetching the result) of a contract preview build.
async fn status(
    input: ContractPreviewStatusInput,
    client: &StudioClient,
) -> Result<Option<crate::shared::check_workflow_poll::PollState>, RoverClientError> {
    let build_id = input.build_id.clone();
    let graph_ref = input.graph_ref.clone();
    let response_data = client
        .post::<ContractPreviewStatusQuery>(input.into())
        .await?;
    let status = require_variant(response_data.graph.map(|graph| graph.variant), &graph_ref)?
        .contract_preview_status
        .ok_or_else(|| RoverClientError::AdhocError {
            msg: format!("No contract preview build found with ID {build_id}."),
        })?;

    use contract_preview_status_query::ContractPreviewStatusQueryGraphVariantContractPreviewStatus as Status;

    let finished = !matches!(status, Status::ContractPreviewAsyncPending);
    Ok(Some(crate::shared::check_workflow_poll::PollState {
        finished,
        target_url: None,
    }))
}

/// Fetch the full result of a previously started contract preview build.
pub async fn result(
    input: ContractPreviewStatusInput,
    client: &StudioClient,
) -> Result<PreviewJobResponse, RoverClientError> {
    let build_id = input.build_id.clone();
    let graph_ref = input.graph_ref.clone();
    let response_data = client
        .post::<ContractPreviewResultQuery>(input.into())
        .await?;
    let status = require_variant(response_data.graph.map(|graph| graph.variant), &graph_ref)?
        .contract_preview_status
        .ok_or_else(|| RoverClientError::AdhocError {
            msg: format!("No contract preview build found with ID {build_id}."),
        })?;

    Ok(map_status_response(graph_ref, build_id, status))
}

/// Continuously poll the status of an already-started contract preview build.
pub async fn poll(
    status_input: ContractPreviewStatusInput,
    client: &StudioClient,
    checks_timeout_seconds: u64,
) -> Result<PreviewJobResponse, RoverClientError> {
    poll_preview_build(
        checks_timeout_seconds,
        &status_input.build_id,
        async || status(status_input.clone(), client).await,
        async || result(status_input.clone(), client).await,
    )
    .await
}

/// Start an async contract preview build then poll until it reaches a
/// terminal state.
pub async fn run(
    input: ContractPreviewInput,
    client: &StudioClient,
    checks_timeout_seconds: u64,
) -> Result<PreviewJobResponse, RoverClientError> {
    let graph_ref = input.graph_ref.clone();
    let started = start(input, client).await?;
    let status_input = ContractPreviewStatusInput {
        graph_ref,
        build_id: started.build_id,
    };
    poll(status_input, client, checks_timeout_seconds).await
}

type StatusUnion =
    contract_preview_result_query::ContractPreviewResultQueryGraphVariantContractPreviewStatus;
type PendingStatus = contract_preview_result_query::ContractPreviewAsyncPendingStatus;

/// Maps the `contractPreviewStatus` union response into the domain
/// `PreviewJobResponse`.
fn map_status_response(
    graph_ref: rover_studio::types::GraphRef,
    build_id: String,
    status: StatusUnion,
) -> PreviewJobResponse {
    match status {
        StatusUnion::ContractPreviewAsyncPending(pending) => PreviewJobResponse {
            graph_ref,
            kind: PreviewKind::Contract,
            build_id: pending.build_id,
            status: match pending.status {
                PendingStatus::PENDING => AsyncBuildStatus::Pending,
                PendingStatus::RUNNING => AsyncBuildStatus::Running,
                PendingStatus::Other(other) => {
                    // Report unknown status directly to the user
                    eprintln!(
                        "warning: received unrecognized contract preview status '{other}'; treating it as still in progress"
                    );
                    AsyncBuildStatus::Running
                }
            },
            api_schema: None,
            supergraph_schema: None,
            errors: Vec::new(),
        },
        StatusUnion::ContractPreviewSuccess(success) => PreviewJobResponse {
            graph_ref,
            kind: PreviewKind::Contract,
            build_id,
            status: AsyncBuildStatus::Success,
            api_schema: Some(success.api_document),
            supergraph_schema: Some(success.core_document),
            errors: Vec::new(),
        },
        StatusUnion::ContractPreviewErrors(failure) => PreviewJobResponse {
            graph_ref,
            kind: PreviewKind::Contract,
            build_id,
            status: AsyncBuildStatus::FilterFailed,
            api_schema: None,
            supergraph_schema: None,
            errors: failure.errors,
        },
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use houston::{Credential, CredentialOrigin};
    use httpmock::prelude::*;
    use reqwest::Client as ReqwestClient;
    use serde_json::json;

    use super::*;
    use crate::shared::ContractFilterConfig;

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

    fn test_status_input(build_id: &str) -> ContractPreviewStatusInput {
        ContractPreviewStatusInput {
            graph_ref: "test-graph@test-variant".parse().unwrap(),
            build_id: build_id.to_string(),
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

        let response = start(test_input(), &test_client(&server.url("/")))
            .await
            .unwrap();

        assert_eq!(response.build_id, "build-123");
        assert_eq!(response.status, AsyncBuildStatus::Pending);
        assert_eq!(response.kind, PreviewKind::Contract);
        assert_eq!(response.graph_ref.to_string(), "test-graph@test-variant");
    }

    #[tokio::test]
    async fn result_maps_pending_running() {
        let server = MockServer::start_async().await;
        server.mock(|when, then| {
            when.method(POST)
                .body_includes("ContractPreviewResultQuery");
            then.status(200).json_body(json!({
                "data": { "graph": { "variant": {
                    "contractPreviewStatus": {
                        "__typename": "ContractPreviewAsyncPending",
                        "buildID": "build-123",
                        "status": "RUNNING"
                    }
                } } }
            }));
        });

        let response = result(
            test_status_input("build-123"),
            &test_client(&server.url("/")),
        )
        .await
        .unwrap();

        assert_eq!(response.status, AsyncBuildStatus::Running);
        assert_eq!(response.build_id, "build-123");
    }

    #[tokio::test]
    async fn result_maps_unrecognized_pending_substatus_to_running() {
        let server = MockServer::start_async().await;
        server.mock(|when, then| {
            when.method(POST)
                .body_includes("ContractPreviewResultQuery");
            then.status(200).json_body(json!({
                "data": { "graph": { "variant": {
                    "contractPreviewStatus": {
                        "__typename": "ContractPreviewAsyncPending",
                        "buildID": "build-123",
                        "status": "SOME_FUTURE_SUBSTATUS"
                    }
                } } }
            }));
        });

        let response = result(
            test_status_input("build-123"),
            &test_client(&server.url("/")),
        )
        .await
        .unwrap();

        assert_eq!(response.status, AsyncBuildStatus::Running);
        assert_eq!(response.build_id, "build-123");
    }

    #[tokio::test]
    async fn result_maps_success() {
        let server = MockServer::start_async().await;
        server.mock(|when, then| {
            when.method(POST)
                .body_includes("ContractPreviewResultQuery");
            then.status(200).json_body(json!({
                "data": { "graph": { "variant": {
                    "contractPreviewStatus": {
                        "__typename": "ContractPreviewSuccess",
                        "apiDocument": "type Query { filtered: String }",
                        "coreDocument": "filtered supergraph"
                    }
                } } }
            }));
        });

        let response = result(
            test_status_input("build-123"),
            &test_client(&server.url("/")),
        )
        .await
        .unwrap();

        assert_eq!(response.status, AsyncBuildStatus::Success);
        assert_eq!(
            response.api_schema,
            Some("type Query { filtered: String }".to_string())
        );
        assert_eq!(
            response.supergraph_schema,
            Some("filtered supergraph".to_string())
        );
    }

    #[tokio::test]
    async fn result_maps_failure() {
        let server = MockServer::start_async().await;
        server.mock(|when, then| {
            when.method(POST)
                .body_includes("ContractPreviewResultQuery");
            then.status(200).json_body(json!({
                "data": { "graph": { "variant": {
                    "contractPreviewStatus": {
                        "__typename": "ContractPreviewErrors",
                        "errors": ["unknown tag 'internal'"],
                        "failedAt": "TO_FILTER_SCHEMA"
                    }
                } } }
            }));
        });

        let response = result(
            test_status_input("build-123"),
            &test_client(&server.url("/")),
        )
        .await
        .unwrap();

        assert_eq!(response.status, AsyncBuildStatus::FilterFailed);
        assert_eq!(response.api_schema, None);
        assert_eq!(response.errors, vec!["unknown tag 'internal'".to_string()]);
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

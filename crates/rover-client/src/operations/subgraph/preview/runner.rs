use graphql_client::*;

use super::types::{
    AsyncBuildStatus, ComposeAndFilterPreviewInput, ComposeAndFilterPreviewStatusInput,
    PreviewJobResponse, PreviewKind,
};
use crate::{
    blocking::StudioClient,
    shared::preview_poll::{poll_preview_build, require_variant},
    RoverClientError,
};

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

/// Start an async compose-and-filter preview job, returning its (pending)
/// status immediately.
pub async fn start(
    input: ComposeAndFilterPreviewInput,
    client: &StudioClient,
) -> Result<PreviewJobResponse, RoverClientError> {
    let graph_ref = input.graph_ref.clone();
    let response_data = client
        .post::<ComposeAndFilterPreviewAsyncMutation>(input.into())
        .await?;
    let build_id = require_variant(response_data.graph.map(|graph| graph.variant), &graph_ref)?
        .compose_and_filter_preview_async
        .build_id;
    Ok(PreviewJobResponse {
        graph_ref,
        kind: PreviewKind::Subgraph,
        build_id,
        status: AsyncBuildStatus::Pending,
        api_schema: None,
        supergraph_schema: None,
        errors: Vec::new(),
    })
}

/// Fetch the full result of a previously started compose-and-filter preview build.
pub async fn result(
    input: ComposeAndFilterPreviewStatusInput,
    client: &StudioClient,
) -> Result<PreviewJobResponse, RoverClientError> {
    let build_id = input.build_id.clone();
    let graph_ref = input.graph_ref.clone();
    let response_data = client
        .post::<ComposeAndFilterPreviewResultQuery>(input.into())
        .await?;
    let status = require_variant(response_data.graph.map(|graph| graph.variant), &graph_ref)?
        .compose_and_filter_preview_status
        .ok_or_else(|| RoverClientError::AdhocError {
            msg: format!("No compose-and-filter preview build found with ID {build_id}."),
        })?;

    Ok(map_status_response(graph_ref, build_id, status))
}

/// Start an async compose-and-filter preview build then poll until it reaches
/// a terminal state.
pub async fn run(
    input: ComposeAndFilterPreviewInput,
    client: &StudioClient,
    checks_timeout_seconds: u64,
) -> Result<PreviewJobResponse, RoverClientError> {
    let graph_ref = input.graph_ref.clone();
    let started = start(input, client).await?;
    let status_input = ComposeAndFilterPreviewStatusInput {
        graph_ref,
        build_id: started.build_id,
    };
    poll(status_input, client, checks_timeout_seconds).await
}

/// Poll an already-started compose-and-filter preview build (via the shared
/// `crate::shared::check_workflow_poll::poll_check_workflow`, same as
/// `rover subgraph check`) until it reaches a terminal state. Split out from
/// `run` so callers that already know the build ID (e.g. the CLI, which
/// prints it to the user right after starting the build) can poll without
/// starting a second build.
pub async fn poll(
    status_input: ComposeAndFilterPreviewStatusInput,
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

type StatusUnion = compose_and_filter_preview_result_query::ComposeAndFilterPreviewResultQueryGraphVariantComposeAndFilterPreviewStatus;
type PendingStatus = compose_and_filter_preview_result_query::ComposeAndFilterPreviewPendingStatus;

/// Maps the `composeAndFilterPreviewStatus` union response into the domain
/// `PreviewJobResponse`. Pulled out of `status` so the mapping (nested
/// `Option`s, the filter-vs-compose-result preference, the different
/// failure shapes) can be unit tested without a real network call.
fn map_status_response(
    graph_ref: rover_studio::types::GraphRef,
    build_id: String,
    status: StatusUnion,
) -> PreviewJobResponse {
    match status {
        StatusUnion::ComposeAndFilterPreviewPending(pending) => PreviewJobResponse {
            graph_ref,
            kind: PreviewKind::Subgraph,
            build_id: pending.build_id,
            status: match pending.status {
                PendingStatus::PENDING => AsyncBuildStatus::Pending,
                PendingStatus::RUNNING => AsyncBuildStatus::Running,
                PendingStatus::Other(other) => {
                    // Report unknown status directly to the user
                    eprintln!(
                        "warning: received unrecognized subgraph preview status '{other}'; treating it as still in progress"
                    );
                    AsyncBuildStatus::Running
                }
            },
            api_schema: None,
            supergraph_schema: None,
            errors: Vec::new(),
        },
        StatusUnion::ComposeAndFilterPreviewSuccess(success) => {
            // The filtered result (if filtering was requested) is what the
            // user actually gets; fall back to the unfiltered compose result
            // when no filter was applied.
            let (api_schema, supergraph_schema) = match success.filter_results {
                Some(filter_results) => (
                    filter_results.api_schema_document,
                    filter_results.supergraph_schema_document,
                ),
                None => (
                    success.compose_results.api_schema_document,
                    success.compose_results.supergraph_schema_document,
                ),
            };
            PreviewJobResponse {
                graph_ref,
                kind: PreviewKind::Subgraph,
                build_id,
                status: AsyncBuildStatus::Success,
                api_schema: Some(api_schema),
                supergraph_schema: Some(supergraph_schema),
                errors: Vec::new(),
            }
        }
        StatusUnion::ComposeAndFilterPreviewComposeFailure(failure) => PreviewJobResponse {
            graph_ref,
            kind: PreviewKind::Subgraph,
            build_id,
            status: AsyncBuildStatus::ComposeFailed,
            api_schema: None,
            supergraph_schema: None,
            errors: failure
                .compose_errors
                .into_iter()
                .map(|error| error.message)
                .collect(),
        },
        StatusUnion::ComposeAndFilterPreviewFilterFailure(failure) => PreviewJobResponse {
            graph_ref,
            kind: PreviewKind::Subgraph,
            build_id,
            status: AsyncBuildStatus::FilterFailed,
            api_schema: Some(failure.compose_results.api_schema_document),
            supergraph_schema: Some(failure.compose_results.supergraph_schema_document),
            errors: failure
                .filter_errors
                .into_iter()
                .map(|error| error.message)
                .collect(),
        },
    }
}

/// Check the status of a previously started compose-and-filter preview build
/// using the lightweight, `__typename`-only selection. Used as
/// `poll_check_workflow`'s repeated `poll_status`, so that polling a
/// long-running build doesn't re-fetch its full (potentially large) schema
/// documents every few seconds.
async fn status(
    input: ComposeAndFilterPreviewStatusInput,
    client: &StudioClient,
) -> Result<Option<crate::shared::check_workflow_poll::PollState>, RoverClientError> {
    let build_id = input.build_id.clone();
    let graph_ref = input.graph_ref.clone();
    let response_data = client
        .post::<ComposeAndFilterPreviewStatusQuery>(input.into())
        .await?;
    // Unlike `SubgraphCheckWorkflowStatusQuery`, there's no eventual-consistency
    // lag to accommodate here: once `composeAndFilterPreviewAsync` returns a
    // build ID, that build is immediately pollable, so a missing graph/variant/
    // build is a genuine error rather than "not ready yet".
    let status = require_variant(response_data.graph.map(|graph| graph.variant), &graph_ref)?
        .compose_and_filter_preview_status
        .ok_or_else(|| RoverClientError::AdhocError {
            msg: format!("No compose-and-filter preview build found with ID {build_id}."),
        })?;

    use compose_and_filter_preview_status_query::ComposeAndFilterPreviewStatusQueryGraphVariantComposeAndFilterPreviewStatus as Status;

    let finished = !matches!(status, Status::ComposeAndFilterPreviewPending);
    Ok(Some(crate::shared::check_workflow_poll::PollState {
        finished,
        target_url: None,
    }))
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use houston::{Credential, CredentialOrigin};
    use httpmock::prelude::*;
    use reqwest::Client as ReqwestClient;
    use serde_json::json;

    use super::*;

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

    fn test_status_input(build_id: &str) -> ComposeAndFilterPreviewStatusInput {
        ComposeAndFilterPreviewStatusInput {
            graph_ref: "test-graph@test-variant".parse().unwrap(),
            build_id: build_id.to_string(),
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

        let response = start(test_input(), &test_client(&server.url("/")))
            .await
            .unwrap();

        assert_eq!(response.build_id, "build-123");
        assert_eq!(response.status, AsyncBuildStatus::Pending);
        assert_eq!(response.graph_ref.to_string(), "test-graph@test-variant");
        assert_eq!(response.api_schema, None);
        assert_eq!(response.supergraph_schema, None);
    }

    #[tokio::test]
    async fn result_maps_pending_running() {
        let server = MockServer::start_async().await;
        server.mock(|when, then| {
            when.method(POST)
                .body_includes("ComposeAndFilterPreviewResultQuery");
            then.status(200).json_body(json!({
                "data": { "graph": { "variant": {
                    "composeAndFilterPreviewStatus": {
                        "__typename": "ComposeAndFilterPreviewPending",
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
                .body_includes("ComposeAndFilterPreviewResultQuery");
            then.status(200).json_body(json!({
                "data": { "graph": { "variant": {
                    "composeAndFilterPreviewStatus": {
                        "__typename": "ComposeAndFilterPreviewPending",
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
    async fn result_prefers_filter_results_over_compose_results_on_success() {
        let server = MockServer::start_async().await;
        server.mock(|when, then| {
            when.method(POST)
                .body_includes("ComposeAndFilterPreviewResultQuery");
            then.status(200).json_body(json!({
                "data": { "graph": { "variant": {
                    "composeAndFilterPreviewStatus": {
                        "__typename": "ComposeAndFilterPreviewSuccess",
                        "composeResults": {
                            "apiSchemaDocument": "type Query { unfiltered: String }",
                            "supergraphSchemaDocument": "unfiltered supergraph"
                        },
                        "filterResults": {
                            "apiSchemaDocument": "type Query { filtered: String }",
                            "supergraphSchemaDocument": "filtered supergraph"
                        }
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
    async fn result_falls_back_to_compose_results_when_filtering_was_skipped() {
        let server = MockServer::start_async().await;
        server.mock(|when, then| {
            when.method(POST)
                .body_includes("ComposeAndFilterPreviewResultQuery");
            then.status(200).json_body(json!({
                "data": { "graph": { "variant": {
                    "composeAndFilterPreviewStatus": {
                        "__typename": "ComposeAndFilterPreviewSuccess",
                        "composeResults": {
                            "apiSchemaDocument": "type Query { composed: String }",
                            "supergraphSchemaDocument": "composed supergraph"
                        },
                        "filterResults": null
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
            Some("type Query { composed: String }".to_string())
        );
    }

    #[tokio::test]
    async fn result_maps_compose_failure() {
        let server = MockServer::start_async().await;
        server.mock(|when, then| {
            when.method(POST)
                .body_includes("ComposeAndFilterPreviewResultQuery");
            then.status(200).json_body(json!({
                "data": { "graph": { "variant": {
                    "composeAndFilterPreviewStatus": {
                        "__typename": "ComposeAndFilterPreviewComposeFailure",
                        "composeErrors": [
                            { "message": "subgraph schema is invalid", "code": "INVALID_GRAPHQL", "failedStep": "PARSE" }
                        ]
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

        assert_eq!(response.status, AsyncBuildStatus::ComposeFailed);
        assert_eq!(response.api_schema, None);
        assert_eq!(
            response.errors,
            vec!["subgraph schema is invalid".to_string()]
        );
    }

    #[tokio::test]
    async fn result_maps_filter_failure_and_still_returns_the_composed_schema() {
        let server = MockServer::start_async().await;
        server.mock(|when, then| {
            when.method(POST)
                .body_includes("ComposeAndFilterPreviewResultQuery");
            then.status(200).json_body(json!({
                "data": { "graph": { "variant": {
                    "composeAndFilterPreviewStatus": {
                        "__typename": "ComposeAndFilterPreviewFilterFailure",
                        "composeResults": {
                            "apiSchemaDocument": "type Query { composed: String }",
                            "supergraphSchemaDocument": "composed supergraph"
                        },
                        "filterErrors": [
                            { "message": "unknown tag 'internal'", "failedStep": "VALIDATE" }
                        ]
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
        // The compose result is still surfaced even though filtering failed.
        assert_eq!(
            response.api_schema,
            Some("type Query { composed: String }".to_string())
        );
        assert_eq!(response.errors, vec!["unknown tag 'internal'".to_string()]);
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

        assert_eq!(response.status, AsyncBuildStatus::Success);
        assert_eq!(response.build_id, "build-123");
        assert_eq!(
            response.api_schema,
            Some("type Query { hi: String }".to_string())
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

        assert!(
            matches!(err, RoverClientError::AdhocError { .. }),
            "expected an immediate AdhocError, got {err:?}"
        );
    }
}

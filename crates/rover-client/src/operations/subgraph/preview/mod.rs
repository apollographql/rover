mod service;
mod types;

pub use service::{ComposeAndFilterPreviewResult, ComposeAndFilterPreviewStart};
use tower::{Service, ServiceExt};
pub use types::*;

use crate::{blocking::StudioClient, shared::preview_poll::poll_preview_build, RoverClientError};

/// Start an async compose-and-filter preview job, returning its (pending)
/// status immediately.
pub async fn start(
    input: ComposeAndFilterPreviewInput,
    client: &StudioClient,
) -> Result<PreviewJobResponse, RoverClientError> {
    let mut service = ComposeAndFilterPreviewStart::new(
        client
            .studio_graphql_service()
            .map_err(|err| RoverClientError::ServiceReady(Box::new(err)))?,
    );
    let service = service.ready().await?;
    service.call(input).await
}

/// Check the status (without fetching the result) of a compose-and-filter
/// preview build.
async fn status(
    input: ComposeAndFilterPreviewStatusInput,
    client: &StudioClient,
) -> Result<Option<crate::shared::check_workflow_poll::PollState>, RoverClientError> {
    let mut service = service::ComposeAndFilterPreviewStatus::new(
        client
            .studio_graphql_service()
            .map_err(|err| RoverClientError::ServiceReady(Box::new(err)))?,
    );
    let service = service.ready().await?;
    service.call(input).await
}

/// Fetch the full result of a previously started compose-and-filter preview build.
pub async fn result(
    input: ComposeAndFilterPreviewStatusInput,
    client: &StudioClient,
) -> Result<PreviewJobResponse, RoverClientError> {
    let mut service = ComposeAndFilterPreviewResult::new(
        client
            .studio_graphql_service()
            .map_err(|err| RoverClientError::ServiceReady(Box::new(err)))?,
    );
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
    let started = start(input, client).await?;
    let status_input = ComposeAndFilterPreviewStatusInput {
        graph_ref,
        build_id: started.build_id,
    };
    poll(status_input, client, checks_timeout_seconds).await
}

/// Continuously poll the status of an already-started compose-and-filter
/// preview build.
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

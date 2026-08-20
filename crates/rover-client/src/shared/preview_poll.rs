use rover_studio::types::GraphRef;

use crate::{
    shared::check_workflow_poll::{poll_check_workflow, PollState},
    RoverClientError,
};

/// Resolves the `graph { variant { ... } }` two-level lookup shared by every
/// preview mutation/query (`composeAndFilterPreviewAsync`,
/// `contractPreviewAsync`, and their status queries): both levels are
/// optional in the generated response type, and either being absent means
/// the graph/variant wasn't found.
// TODO(preview): drop this `allow` once the contract/subgraph preview
// operations (stacked on top of this PR) call it.
#[allow(dead_code)]
pub(crate) fn require_variant<V>(
    variant: Option<Option<V>>,
    graph_ref: &GraphRef,
) -> Result<V, RoverClientError> {
    variant
        .flatten()
        .ok_or_else(|| RoverClientError::GraphNotFound {
            graph_ref: graph_ref.clone(),
        })
}

/// Polls a preview build to completion then remaps its error messages to
/// preview-appropriate wording (but `APOLLO_CHECKS_TIMEOUT_SECONDS` still
/// controls polling waits, so the remapped message keeps a pointer to it,
/// along with `--build-id` for checking status later).
// TODO(preview): drop this `allow` once the contract/subgraph preview
// operations (stacked on top of this PR) call it.
#[allow(dead_code)]
pub(crate) async fn poll_preview_build<T>(
    checks_timeout_seconds: u64,
    build_id: &str,
    poll_status: impl AsyncFnMut() -> Result<Option<PollState>, RoverClientError>,
    fetch_result: impl AsyncFnOnce() -> Result<T, RoverClientError>,
) -> Result<T, RoverClientError> {
    poll_check_workflow(checks_timeout_seconds, poll_status, fetch_result)
        .await
        .map_err(|err| map_preview_errors(build_id, err))
}

fn map_preview_errors(build_id: &str, err: RoverClientError) -> RoverClientError {
    match err {
        RoverClientError::ChecksTimeoutError { .. } => RoverClientError::AdhocError {
            msg: format!(
                "Timed out waiting for preview {build_id}, check back with `--build-id {build_id}`, or raise APOLLO_CHECKS_TIMEOUT_SECONDS"
            ),
        },
        RoverClientError::CheckWorkflowResultUnavailable { source, .. } => {
            RoverClientError::AdhocError {
                msg: format!(
                    "Job {build_id} finished, but could not fetch the result: {source}"
                ),
            }
        }
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn poll_preview_build_fetches_result_once_finished() {
        let out = poll_preview_build(
            30,
            "build-1",
            async || {
                Ok(Some(PollState {
                    finished: true,
                    target_url: None,
                }))
            },
            async || Ok::<_, RoverClientError>("schema".to_string()),
        )
        .await;
        assert_eq!(out.unwrap(), "schema");
    }

    #[tokio::test]
    async fn poll_preview_build_remaps_timeout_errors() {
        let out = poll_preview_build::<i32>(
            0,
            "build-1",
            async || Ok(None),
            async || panic!("fetch must not run when the build never finishes"),
        )
        .await;
        match out {
            Err(RoverClientError::AdhocError { msg }) => {
                assert!(msg.contains("build-1"));
                assert!(msg.contains("--build-id build-1"));
            }
            other => panic!("expected AdhocError, got {other:?}"),
        }
    }

    #[test]
    fn require_variant_resolves_present_variant() {
        let graph_ref: GraphRef = "test-graph@test-variant".parse().unwrap();
        assert_eq!(require_variant(Some(Some(42)), &graph_ref).unwrap(), 42);
    }

    #[test]
    fn require_variant_errors_on_missing_graph() {
        let graph_ref: GraphRef = "test-graph@test-variant".parse().unwrap();
        let err = require_variant::<i32>(None, &graph_ref).unwrap_err();
        assert!(matches!(err, RoverClientError::GraphNotFound { .. }));
    }

    #[test]
    fn require_variant_errors_on_missing_variant() {
        let graph_ref: GraphRef = "test-graph@test-variant".parse().unwrap();
        let err = require_variant::<i32>(Some(None), &graph_ref).unwrap_err();
        assert!(matches!(err, RoverClientError::GraphNotFound { .. }));
    }

    #[test]
    fn maps_checks_timeout_error() {
        let out = map_preview_errors(
            "job-1",
            RoverClientError::ChecksTimeoutError {
                url: Some("https://studio.example/checks/abc".to_string()),
            },
        );
        match out {
            RoverClientError::AdhocError { msg } => {
                assert!(msg.contains("job-1"), "expected build id in: {msg}");
                assert!(
                    msg.contains("--build-id"),
                    "expected a pointer to re-checking the build in: {msg}"
                );
                assert!(
                    msg.contains("APOLLO_CHECKS_TIMEOUT_SECONDS"),
                    "expected a pointer to the timeout env var in: {msg}"
                );
                assert!(
                    !msg.contains("check workflow"),
                    "expected check-workflow-specific wording to be stripped, got: {msg}"
                );
            }
            other => panic!("expected AdhocError, got {other:?}"),
        }
    }

    #[test]
    fn maps_check_workflow_result_unavailable_error() {
        let out = map_preview_errors(
            "job-1",
            RoverClientError::CheckWorkflowResultUnavailable {
                url: Some("https://studio.example/checks/abc".to_string()),
                source: Box::new(RoverClientError::AdhocError {
                    msg: "boom".to_string(),
                }),
            },
        );
        match out {
            RoverClientError::AdhocError { msg } => {
                assert!(msg.contains("job-1"));
                assert!(msg.contains("boom"));
                assert!(
                    !msg.contains("Studio"),
                    "expected check-specific wording to be stripped, got: {msg}"
                );
            }
            other => panic!("expected AdhocError, got {other:?}"),
        }
    }

    #[test]
    fn passes_through_other_errors_unchanged() {
        let out = map_preview_errors("job-1", RoverClientError::RateLimitExceeded);
        assert!(matches!(out, RoverClientError::RateLimitExceeded));
    }
}

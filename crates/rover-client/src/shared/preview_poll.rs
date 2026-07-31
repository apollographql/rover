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

/// Polls a preview build to completion (via the shared
/// `check_workflow_poll::poll_check_workflow`, same as `rover subgraph
/// check`), then remaps its error messages to preview-appropriate wording.
/// `poll_check_workflow`'s own error messages assume a check workflow (e.g.
/// mentioning `APOLLO_CHECKS_TIMEOUT_SECONDS` and Studio check pages),
/// which would be misleading here.
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
            msg: format!("Timed out waiting for job {build_id} to complete."),
        },
        RoverClientError::CheckWorkflowResultUnavailable { source, .. } => {
            RoverClientError::AdhocError {
                msg: format!(
                    "Job {build_id} finished, but Rover couldn't fetch the result: {source}"
                ),
            }
        }
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
                assert!(msg.contains("job-1"));
                assert!(
                    !msg.contains("APOLLO_CHECKS_TIMEOUT_SECONDS"),
                    "expected check-specific wording to be stripped, got: {msg}"
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

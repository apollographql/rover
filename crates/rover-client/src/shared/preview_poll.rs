use rover_studio::types::GraphRef;

use crate::{
    shared::check_workflow_poll::{poll_check_workflow, PollState},
    RoverClientError,
};

/// Reports the `graph { variant { ... } }` two-level lookup shared by every
/// preview mutation/query (`composeAndFilterPreviewAsync`,
/// `contractPreviewAsync`, and their status queries) as not found. Callers
/// flatten the generated response type's `Option<Option<V>>` themselves
/// (e.g. `graph.and_then(|g| g.variant)`) before calling this.
pub(crate) fn require_variant<V>(
    variant: Option<V>,
    graph_ref: &GraphRef,
) -> Result<V, RoverClientError> {
    variant.ok_or_else(|| RoverClientError::GraphNotFound {
        graph_ref: graph_ref.clone(),
    })
}

/// Polls a preview build to completion then remaps its error messages to
/// preview-appropriate wording (but `APOLLO_CHECKS_TIMEOUT_SECONDS` still
/// controls polling waits, so the remapped message keeps a pointer to it,
/// along with `--build-id` for checking status later).
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
        RoverClientError::ChecksTimeoutError { .. } => RoverClientError::PreviewTimeoutError {
            build_id: build_id.to_string(),
        },
        RoverClientError::CheckWorkflowResultUnavailable { source, .. } => {
            RoverClientError::PreviewResultUnavailable {
                build_id: build_id.to_string(),
                source,
            }
        }
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use speculoos::prelude::*;

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
        assert_that!(out).is_ok().is_equal_to("schema".to_string());
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
        let err = out.unwrap_err();
        assert_that!(matches!(err, RoverClientError::PreviewTimeoutError { .. })).is_true();
        assert_that!(&err.to_string()).contains("build-1");
        assert_that!(&err.to_string()).contains("--build-id build-1");
    }

    #[test]
    fn require_variant_resolves_present_variant() {
        let graph_ref: GraphRef = "test-graph@test-variant".parse().unwrap();
        assert_that!(require_variant(Some(42), &graph_ref))
            .is_ok()
            .is_equal_to(42);
    }

    #[test]
    fn require_variant_errors_on_missing_variant() {
        let graph_ref: GraphRef = "test-graph@test-variant".parse().unwrap();
        let err = require_variant::<i32>(None, &graph_ref).unwrap_err();
        assert_that!(matches!(err, RoverClientError::GraphNotFound { .. })).is_true();
    }

    #[test]
    fn maps_checks_timeout_error() {
        let out = map_preview_errors(
            "job-1",
            RoverClientError::ChecksTimeoutError {
                url: Some("https://studio.example/checks/abc".to_string()),
            },
        );
        assert_that!(matches!(out, RoverClientError::PreviewTimeoutError { .. })).is_true();
        let message = out.to_string();
        assert_that!(&message).contains("job-1");
        assert_that!(&message).contains("--build-id job-1");
        assert_that!(&message).contains("APOLLO_CHECKS_TIMEOUT_SECONDS");
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
        assert_that!(matches!(
            out,
            RoverClientError::PreviewResultUnavailable { .. }
        ))
        .is_true();
        assert_that!(&out.to_string()).contains("job-1");
        let source = out
            .source()
            .expect("PreviewResultUnavailable should carry the original error as its source");
        assert_that!(&source.to_string()).contains("boom");
    }

    #[test]
    fn passes_through_other_errors_unchanged() {
        let out = map_preview_errors("job-1", RoverClientError::RateLimitExceeded);
        assert_that!(matches!(out, RoverClientError::RateLimitExceeded)).is_true();
    }
}

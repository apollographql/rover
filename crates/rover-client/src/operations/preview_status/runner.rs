use super::types::{PreviewJobResponse, PreviewStatusInput};
use crate::{blocking::StudioClient, shared::check_workflow_poll::PollState, RoverClientError};

/// Check the results of a preview job
pub async fn results(
    _input: PreviewStatusInput,
    _client: &StudioClient,
) -> Result<PreviewJobResponse, RoverClientError> {
    Err(RoverClientError::AdhocError {
        msg: "`previewStatus` is not yet available in the platform API schema vendored by Rover. This command is a skeleton awaiting the async contracts API.".to_string(),
    })
}

/// Check the status of a preview job.
///
/// Once implemented, this should map `status` into `PollState`
/// Should return `Ok(None)` if the job isn't reportable yet (e.g. immediately
/// after `contractPreviewAsync`/`composeAndFilterPreviewAsync` returns,
/// before the job is queryable) — mirroring
/// `SubgraphCheckWorkflowStatusQuery`'s handling of the same kind of
/// eventual-consistency lag.
pub(crate) async fn status(
    _input: PreviewStatusInput,
    _client: &StudioClient,
) -> Result<Option<PollState>, RoverClientError> {
    Err(RoverClientError::AdhocError {
        msg: "`previewStatus` is not yet available in the platform API schema vendored by Rover. This command is a skeleton awaiting the async contracts API.".to_string(),
    })
}

/// Maps a preview job's errors to error messages. Because the preview job
/// reuses `poll_check_workflow`, some error messages are misleading. Any
/// error that would produce a confusing message is trapped here and converted
/// into an AdhocError, which will pass through the wrapping error message
/// generation unchanged.
pub(crate) fn map_preview_errors(job_id: &str, err: RoverClientError) -> RoverClientError {
    match err {
        RoverClientError::ChecksTimeoutError { .. } => RoverClientError::AdhocError {
            msg: format!("Timed out waiting for job {job_id} to complete."),
        },
        RoverClientError::CheckWorkflowResultUnavailable { source, .. } => {
            RoverClientError::AdhocError {
                msg: format!(
                    "Job {job_id} finished, but Rover couldn't fetch the result: {source}"
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

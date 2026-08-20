use std::{
    thread,
    time::{Duration, Instant},
};

use crate::RoverClientError;

/// The outcome of a single check-workflow status poll.
pub(crate) struct PollState {
    /// Whether the workflow has left the `PENDING` state.
    pub finished: bool,
    /// The Studio URL for the check, if known yet.
    pub target_url: Option<String>,
}

/// Polls a lightweight, status-only query until the workflow finishes (or the
/// timeout elapses), then fetches the full result exactly once to avoid polling the full
/// result on every iteration makes the response balloon for large schemas, which
/// can make the poll fail or time out.
///
/// `poll_status` returns `Ok(None)` when the workflow isn't reportable yet (so we
/// keep polling). A transient `Err` (per [`RoverClientError::is_transient`]) is
/// logged and retried until the timeout; any other `Err` surfaces immediately.
pub(crate) async fn poll_check_workflow<T>(
    checks_timeout_seconds: u64,
    mut poll_status: impl AsyncFnMut() -> Result<Option<PollState>, RoverClientError>,
    fetch_result: impl AsyncFnOnce() -> Result<T, RoverClientError>,
) -> Result<T, RoverClientError> {
    let now = Instant::now();
    let mut url: Option<String> = None;
    loop {
        match poll_status().await {
            Ok(None) => {}
            Ok(Some(state)) => {
                url = state.target_url.or(url);
                if state.finished {
                    break;
                }
            }
            Err(e) if e.is_transient() => {
                eprintln!("error while checking status: {e}\nretrying...");
            }
            Err(e) => return Err(e),
        }
        if now.elapsed() > Duration::from_secs(checks_timeout_seconds) {
            return Err(RoverClientError::ChecksTimeoutError { url });
        }
        thread::sleep(Duration::from_secs(5));
    }

    fetch_result()
        .await
        .map_err(|source| RoverClientError::CheckWorkflowResultUnavailable {
            url,
            source: Box::new(source),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn some_error() -> RoverClientError {
        RoverClientError::AdhocError {
            msg: "boom".to_string(),
        }
    }

    #[tokio::test]
    async fn fetches_and_returns_result_once_finished() {
        let out = poll_check_workflow(
            30,
            async || {
                Ok(Some(PollState {
                    finished: true,
                    target_url: Some("https://studio.example/checks/abc".to_string()),
                }))
            },
            async || Ok::<_, RoverClientError>(42),
        )
        .await;
        assert!(matches!(out, Ok(42)));
    }

    #[tokio::test]
    async fn wraps_failed_result_fetch_and_keeps_url() {
        let out = poll_check_workflow(
            30,
            async || {
                Ok(Some(PollState {
                    finished: true,
                    target_url: Some("https://studio.example/checks/abc".to_string()),
                }))
            },
            async || Err::<i32, _>(some_error()),
        )
        .await;
        match out {
            Err(RoverClientError::CheckWorkflowResultUnavailable { url, .. }) => {
                assert_eq!(url, Some("https://studio.example/checks/abc".to_string()));
            }
            other => panic!("expected CheckWorkflowResultUnavailable, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn fails_fast_on_non_transient_poll_error() {
        // A non-transient error must surface immediately (not retry, not time out),
        // even with a generous timeout.
        let out = poll_check_workflow::<i32>(
            300,
            async || Err(some_error()), // AdhocError is not transient
            async || panic!("fetch must not run when polling fails fatally"),
        )
        .await;
        assert!(
            matches!(out, Err(RoverClientError::AdhocError { .. })),
            "expected the non-transient error to surface immediately, got {out:?}"
        );
    }

    #[tokio::test]
    async fn retries_transient_poll_error_until_timeout() {
        // A transient error is retried; with a zero budget that means we log it and
        // then time out, rather than surfacing the transient error itself.
        let out = poll_check_workflow::<i32>(
            0,
            async || Err(RoverClientError::RateLimitExceeded), // transient
            async || panic!("fetch must not run when the workflow never finishes"),
        )
        .await;
        assert!(
            matches!(out, Err(RoverClientError::ChecksTimeoutError { .. })),
            "expected a transient error to be retried until timeout, got {out:?}"
        );
    }

    #[tokio::test]
    async fn times_out_carrying_last_known_url() {
        // checks_timeout_seconds = 0 means the budget is already spent after the
        // first (still-pending) poll, so we never sleep and never fetch.
        let out = poll_check_workflow::<i32>(
            0,
            async || {
                Ok(Some(PollState {
                    finished: false,
                    target_url: Some("https://studio.example/checks/abc".to_string()),
                }))
            },
            async || panic!("fetch must not run when the workflow never finishes"),
        )
        .await;
        match out {
            Err(RoverClientError::ChecksTimeoutError { url }) => {
                assert_eq!(url, Some("https://studio.example/checks/abc".to_string()));
            }
            other => panic!("expected ChecksTimeoutError, got {other:?}"),
        }
    }
}

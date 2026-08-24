//! A [`tower::retry::Policy`] for polling: keep calling until the response
//! reports it's finished, sleeping `interval` between attempts, and give up
//! with a caller-supplied error once `timeout` has elapsed since the policy
//! was constructed.
//!
//! Plug this into [`tower::retry::Retry`] (e.g. via `ServiceBuilder::retry`)
//! the same way any other [`tower::retry::Policy`] is used for per-request
//! retries -- "polling" here is just a retry policy whose retry condition is
//! "the response isn't finished yet" instead of "the call failed".

use std::time::Duration;

use tokio::time::{Instant, Sleep};
use tower::retry::Policy;

/// Implemented by a poll status response to report whether polling should
/// stop.
pub trait PollOutcome {
    fn is_finished(&self) -> bool;
}

impl PollOutcome for bool {
    fn is_finished(&self) -> bool {
        *self
    }
}

/// See the module docs.
#[derive(Clone)]
pub struct PollRetryPolicy<F> {
    interval: Duration,
    deadline: Instant,
    on_timeout: F,
}

impl<F> PollRetryPolicy<F> {
    /// `timeout` is measured from the moment this policy is constructed, so
    /// build it immediately before starting to poll (not once and reused
    /// across multiple polls).
    pub fn new(interval: Duration, timeout: Duration, on_timeout: F) -> Self {
        Self {
            interval,
            deadline: Instant::now() + timeout,
            on_timeout,
        }
    }
}

impl<Req, Res, E, F> Policy<Req, Res, E> for PollRetryPolicy<F>
where
    Req: Clone,
    Res: PollOutcome,
    F: Fn() -> E + Clone,
{
    type Future = Sleep;

    fn retry(&mut self, _req: &mut Req, result: &mut Result<Res, E>) -> Option<Self::Future> {
        match result {
            Ok(response) if response.is_finished() => None,
            Err(_) => None,
            Ok(_not_finished) => {
                if Instant::now() >= self.deadline {
                    *result = Err((self.on_timeout)());
                    None
                } else {
                    Some(tokio::time::sleep(self.interval))
                }
            }
        }
    }

    fn clone_request(&mut self, req: &Req) -> Option<Req> {
        Some(req.clone())
    }
}

#[cfg(test)]
mod tests {
    use rstest::fixture;
    use speculoos::prelude::*;

    use super::*;

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct TestError(&'static str);

    #[fixture]
    fn policy() -> PollRetryPolicy<fn() -> TestError> {
        PollRetryPolicy::new(Duration::from_secs(5), Duration::from_secs(30), || {
            TestError("timed out")
        })
    }

    #[tokio::test]
    async fn retry_returns_none_when_finished() {
        let mut policy = policy();
        let mut result: Result<bool, TestError> = Ok(true);

        let decision = policy.retry(&mut (), &mut result);

        assert_that!(decision).is_none();
        assert_that!(result).is_ok().is_equal_to(true);
    }

    #[tokio::test]
    async fn retry_returns_none_on_error_without_retrying() {
        let mut policy = policy();
        let mut result: Result<bool, TestError> = Err(TestError("boom"));

        let decision = policy.retry(&mut (), &mut result);

        assert_that!(decision).is_none();
        assert_that!(result).is_err().is_equal_to(TestError("boom"));
    }

    #[tokio::test]
    async fn retry_schedules_another_attempt_when_not_finished_and_time_remains() {
        let mut policy = policy();
        let mut result: Result<bool, TestError> = Ok(false);

        let decision = policy.retry(&mut (), &mut result);

        assert_that!(decision).is_some();
        // Untouched while there's still time on the clock.
        assert_that!(result).is_ok().is_equal_to(false);
    }

    #[tokio::test(start_paused = true)]
    async fn retry_rewrites_the_result_to_a_timeout_error_once_the_deadline_passes() {
        let mut policy = policy();
        tokio::time::advance(Duration::from_secs(31)).await;
        let mut result: Result<bool, TestError> = Ok(false);

        let decision = policy.retry(&mut (), &mut result);

        assert_that!(decision).is_none();
        assert_that!(result)
            .is_err()
            .is_equal_to(TestError("timed out"));
    }

    #[tokio::test]
    async fn clone_request_clones_the_request() {
        let mut policy = policy();
        let req = "build-123".to_string();

        let cloned = Policy::<String, bool, TestError>::clone_request(&mut policy, &req);

        assert_that!(cloned).is_some().is_equal_to(req);
    }
}

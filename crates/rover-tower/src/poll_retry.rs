//! A [`tower::retry::Policy`] for polling until the response indicates it
//! completed, sleeping `interval` between attempts, and give up with an
//! error if `timeout` elapses first.
//!
//! Use with a Retry [`tower::retry::Policy`] for per-request retries. Polling
//! is just a retry policy whose retry condition is "not finished yet" instead
//! of "the call failed".
//!
//! [`PollRetryPolicy`] doesn't know how to build a domain-specific timeout
//! error itself -- that would make it a passed-in closure baked into the
//! policy. Instead it produces a generic [`PollError::TimedOut`], and the
//! caller composes an ordinary [`tower::util::MapErrLayer`] on top to turn
//! that into whatever error their operation needs:
//!
//! ```ignore
//! let mut service = ServiceBuilder::new()
//!     .map_err(|err: PollError<MyError>| match err {
//!         PollError::TimedOut => MyError::Timeout { .. },
//!         PollError::Inner(e) => e,
//!     })
//!     .layer(RetryLayer::new(PollRetryPolicy::new(interval, timeout)))
//!     .map_err(PollError::Inner)
//!     .service(inner);
//! ```

use std::time::Duration;

use tokio::time::{Instant, Sleep};
use tower::retry::Policy;

/// Whether a poll status response indicates the operation is done, or should
/// be polled again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SimplePollOutcome {
    Complete,
    Incomplete,
}

/// Implemented by a poll status response to stop polling.
pub trait PollOutcome {
    fn poll_outcome(&self) -> SimplePollOutcome;
}

impl PollOutcome for SimplePollOutcome {
    fn poll_outcome(&self) -> SimplePollOutcome {
        *self
    }
}

/// The error a [`PollRetryPolicy`]-driven retry produces: either the wrapped
/// service failed for its own reasons, or the poll deadline elapsed first.
/// See the module docs for how a caller turns `TimedOut` into a
/// domain-specific error via a composed `MapErrLayer`, rather than passing a
/// closure into the policy itself.
#[derive(Debug, Clone)]
pub enum PollError<E> {
    /// The poll deadline elapsed before the response reported
    /// [`SimplePollOutcome::Complete`].
    TimedOut,
    /// The wrapped service failed for a reason other than timing out.
    Inner(E),
}

/// Controls the behavior of polling. Compose with `tower::util::MapErrLayer::new(PollError::Inner)`
/// (to wrap the inner service's error before this policy sees it) and
/// `tower::retry::RetryLayer::new(policy)` -- see the module docs for the
/// exact `ServiceBuilder` chain.
#[derive(Clone)]
pub struct PollRetryPolicy {
    interval: Duration,
    deadline: Instant,
}

impl PollRetryPolicy {
    /// `timeout` is measured from policy creation, so build it immediately
    /// before polling (do not reuse across multiple polls).
    pub fn new(interval: Duration, timeout: Duration) -> Self {
        Self {
            interval,
            deadline: Instant::now() + timeout,
        }
    }
}

impl<Req, Res, E> Policy<Req, Res, PollError<E>> for PollRetryPolicy
where
    Req: Clone,
    Res: PollOutcome,
{
    type Future = Sleep;

    fn retry(
        &mut self,
        _req: &mut Req,
        result: &mut Result<Res, PollError<E>>,
    ) -> Option<Self::Future> {
        match result {
            Ok(response) if response.poll_outcome() == SimplePollOutcome::Complete => None,
            Err(_) => None,
            Ok(_not_finished) => {
                if Instant::now() >= self.deadline {
                    *result = Err(PollError::TimedOut);
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
    use tower::{retry::RetryLayer, Service, ServiceBuilder, ServiceExt};

    use super::*;

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct TestError(&'static str);

    #[fixture]
    fn policy() -> PollRetryPolicy {
        PollRetryPolicy::new(Duration::from_secs(5), Duration::from_secs(30))
    }

    #[tokio::test]
    async fn retry_returns_none_when_finished() {
        let mut policy = policy();
        let mut result: Result<SimplePollOutcome, PollError<TestError>> =
            Ok(SimplePollOutcome::Complete);

        let decision = policy.retry(&mut (), &mut result);

        assert_that!(decision).is_none();
        assert_that!(result)
            .is_ok()
            .is_equal_to(SimplePollOutcome::Complete);
    }

    #[tokio::test]
    async fn retry_returns_none_on_error_without_retrying() {
        let mut policy = policy();
        let mut result: Result<SimplePollOutcome, PollError<TestError>> =
            Err(PollError::Inner(TestError("boom")));

        let decision = policy.retry(&mut (), &mut result);

        assert_that!(decision).is_none();
        assert_that!(matches!(result, Err(PollError::Inner(TestError("boom"))))).is_true();
    }

    #[tokio::test(start_paused = true)]
    async fn retry_schedules_another_attempt_when_not_finished_and_time_remains() {
        let mut policy = policy();
        let mut result: Result<SimplePollOutcome, PollError<TestError>> =
            Ok(SimplePollOutcome::Incomplete);

        let decision = policy.retry(&mut (), &mut result);

        let sleep = decision.expect("a retry should be scheduled while time remains");
        assert_that!(sleep.deadline()).is_equal_to(Instant::now() + Duration::from_secs(5));
        // Untouched while there's still time on the clock.
        assert_that!(result)
            .is_ok()
            .is_equal_to(SimplePollOutcome::Incomplete);
    }

    #[tokio::test(start_paused = true)]
    async fn retry_rewrites_the_result_to_timed_out_once_the_deadline_passes() {
        let mut policy = policy();
        tokio::time::advance(Duration::from_secs(31)).await;
        let mut result: Result<SimplePollOutcome, PollError<TestError>> =
            Ok(SimplePollOutcome::Incomplete);

        let decision = policy.retry(&mut (), &mut result);

        assert_that!(decision).is_none();
        assert_that!(matches!(result, Err(PollError::TimedOut))).is_true();
    }

    #[tokio::test]
    async fn clone_request_clones_the_request() {
        let mut policy = policy();
        let req = "build-123".to_string();

        let cloned = Policy::<String, SimplePollOutcome, PollError<TestError>>::clone_request(
            &mut policy,
            &req,
        );

        assert_that!(cloned).is_some().is_equal_to(req);
    }

    #[tokio::test]
    async fn composed_service_retries_until_the_inner_service_reports_done() {
        use std::sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
        };

        let calls = Arc::new(AtomicUsize::new(0));
        let inner = tower::service_fn({
            let calls = calls.clone();
            move |_req: ()| {
                let calls = calls.clone();
                async move {
                    let call = calls.fetch_add(1, Ordering::SeqCst);
                    Ok::<_, TestError>(if call < 2 {
                        SimplePollOutcome::Incomplete
                    } else {
                        SimplePollOutcome::Complete
                    })
                }
            }
        });

        let mut service = ServiceBuilder::new()
            .map_err(|err: PollError<TestError>| match err {
                PollError::TimedOut => TestError("timed out"),
                PollError::Inner(e) => e,
            })
            .layer(RetryLayer::new(PollRetryPolicy::new(
                Duration::from_millis(1),
                Duration::from_secs(30),
            )))
            .map_err(PollError::Inner)
            .service(inner);

        let outcome = service.ready().await.unwrap().call(()).await.unwrap();

        assert_that!(outcome).is_equal_to(SimplePollOutcome::Complete);
        assert_that!(calls.load(Ordering::SeqCst)).is_equal_to(3);
    }

    #[tokio::test(start_paused = true)]
    async fn composed_service_times_out_via_the_composed_map_err_when_never_done() {
        let inner = tower::service_fn(|_req: ()| async {
            Ok::<_, TestError>(SimplePollOutcome::Incomplete)
        });

        let mut service = ServiceBuilder::new()
            .map_err(|err: PollError<TestError>| match err {
                PollError::TimedOut => TestError("timed out"),
                PollError::Inner(e) => e,
            })
            .layer(RetryLayer::new(PollRetryPolicy::new(
                Duration::from_secs(5),
                Duration::from_secs(30),
            )))
            .map_err(PollError::Inner)
            .service(inner);

        let call = service.ready().await.unwrap().call(());
        tokio::time::advance(Duration::from_secs(31)).await;
        let err = call.await.unwrap_err();

        assert_that!(err).is_equal_to(TestError("timed out"));
    }
}

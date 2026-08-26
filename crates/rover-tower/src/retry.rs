use std::{
    cell::OnceCell,
    time::{Duration, Instant},
};

use tower::{
    retry::{
        backoff::{Backoff, ExponentialBackoff, ExponentialBackoffMaker, MakeBackoff},
        Policy,
    },
    util::rng::HasherRng,
};

/// A `tower::retry::Policy` that retries any `Err` for as long as `max_elapsed_time` allows,
/// backing off exponentially between attempts. Generic over the request/response/error types, so
/// it can wrap any fallible `Service`/`service_fn` -- not just HTTP calls (for that, prefer
/// `rover_http`'s HTTP-specific `RetryPolicy`, which classifies retryable failures by status
/// code/`HttpServiceError` variant rather than treating every `Err` as retryable).
#[derive(Clone)]
pub struct ExponentialBackoffPolicy {
    start_time: OnceCell<Instant>,
    max_elapsed_time: Duration,
    backoff: ExponentialBackoff,
}

impl ExponentialBackoffPolicy {
    /// Retries for up to `max_elapsed_time`, backing off exponentially starting at 500ms and
    /// capping at 5s between attempts.
    pub fn new(max_elapsed_time: Duration) -> Self {
        let backoff = ExponentialBackoffMaker::new(
            Duration::from_millis(500),
            Duration::from_secs(5),
            0.99,
            HasherRng::default(),
        )
        .expect("valid exponential backoff parameters")
        .make_backoff();
        Self {
            start_time: OnceCell::new(),
            max_elapsed_time,
            backoff,
        }
    }

    fn can_retry(&self) -> bool {
        self.start_time.get_or_init(Instant::now).elapsed() < self.max_elapsed_time
    }
}

impl<Req: Clone, Res, E> Policy<Req, Res, E> for ExponentialBackoffPolicy {
    type Future = tokio::time::Sleep;

    fn retry(&mut self, _req: &mut Req, result: &mut Result<Res, E>) -> Option<Self::Future> {
        if result.is_err() && self.can_retry() {
            Some(self.backoff.next_backoff())
        } else {
            None
        }
    }

    fn clone_request(&mut self, req: &Req) -> Option<Req> {
        Some(req.clone())
    }
}

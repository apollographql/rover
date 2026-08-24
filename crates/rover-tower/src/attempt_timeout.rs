//! Bounds a single attempt at calling a wrapped [`Service`], independent of
//! any outer retry/poll budget. Unlike a typical timeout middleware, a timed
//! out attempt doesn't produce an error -- it produces a caller-supplied
//! fallback *response*, so it composes underneath something like
//! [`crate::poll_retry::PollRetryPolicy`] as just another "not finished yet"
//! result instead of aborting the operation early.
//!
//! This mirrors the existing `WHOAMI_ATTEMPT_TIMEOUT`/
//! `CLIENT_CREDENTIALS_ATTEMPT_TIMEOUT` layering (an inner per-attempt
//! `TimeoutLayer` under an outer `RetryLayer`, so one hung attempt can't
//! consume the whole retry budget), generalized to a case where the "give up
//! on this attempt" outcome needs to be a value rather than an error.

use std::time::Duration;

use tower::{Layer, Service};

/// See the module docs.
pub struct AttemptTimeoutLayer<F> {
    duration: Duration,
    on_timeout: F,
}

impl<F> AttemptTimeoutLayer<F> {
    pub const fn new(duration: Duration, on_timeout: F) -> Self {
        Self {
            duration,
            on_timeout,
        }
    }
}

impl<S, F> Layer<S> for AttemptTimeoutLayer<F>
where
    F: Clone,
{
    type Service = AttemptTimeout<S, F>;

    fn layer(&self, inner: S) -> Self::Service {
        AttemptTimeout {
            inner,
            duration: self.duration,
            on_timeout: self.on_timeout.clone(),
        }
    }
}

/// See the module docs.
#[derive(Clone)]
pub struct AttemptTimeout<S, F> {
    inner: S,
    duration: Duration,
    on_timeout: F,
}

impl<S, Req, F> Service<Req> for AttemptTimeout<S, F>
where
    S: Service<Req> + Send + 'static,
    S::Future: Send + 'static,
    F: Fn() -> S::Response + Clone + Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = crate::ResponseFuture<Result<Self::Response, Self::Error>>;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Req) -> Self::Future {
        let resp = self.inner.call(req);
        let sleep = tokio::time::sleep(self.duration);
        let on_timeout = self.on_timeout.clone();
        Box::pin(async move {
            tokio::select! {
                () = sleep => Ok(on_timeout()),
                result = resp => result,
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use std::convert::Infallible;

    use speculoos::prelude::*;
    use tower::{service_fn, ServiceExt};

    use super::*;

    #[tokio::test(start_paused = true)]
    async fn returns_the_inner_response_when_it_completes_in_time() {
        let mut service = AttemptTimeoutLayer::new(Duration::from_secs(10), || false).layer(
            service_fn(|()| async {
                tokio::time::sleep(Duration::from_secs(1)).await;
                Ok::<bool, Infallible>(true)
            }),
        );

        let result = service.ready().await.unwrap().call(()).await;

        assert_that!(result).is_ok().is_equal_to(true);
    }

    #[tokio::test(start_paused = true)]
    async fn returns_the_fallback_when_the_inner_call_is_too_slow() {
        let mut service = AttemptTimeoutLayer::new(Duration::from_secs(1), || false).layer(
            service_fn(|()| async {
                tokio::time::sleep(Duration::from_secs(10)).await;
                Ok::<bool, Infallible>(true)
            }),
        );

        let result = service.ready().await.unwrap().call(()).await;

        // The fallback, not the inner service's eventual (never-awaited) true.
        assert_that!(result).is_ok().is_equal_to(false);
    }
}

//! rover-http specific constructs for timing out a request

use std::time::Duration;

use rover_tower::ResponseFuture;
use tower::{Layer, Service};

use crate::HttpServiceError;

/// [`tower::Layer`] for that wraps a Service in a timeout
pub struct TimeoutLayer {
    timeout: Duration,
}

impl TimeoutLayer {
    /// Creates a new TimeoutLayer given a [`Duration`]
    pub const fn new(timeout: Duration) -> TimeoutLayer {
        TimeoutLayer { timeout }
    }
}

impl<S> Layer<S> for TimeoutLayer {
    type Service = Timeout<S>;
    fn layer(&self, inner: S) -> Self::Service {
        Timeout::new(inner, self.timeout)
    }
}

/// Object that wraps another [`Service`] in a timeout
#[derive(Clone, Debug)]
pub struct Timeout<S> {
    inner: S,
    timeout: Duration,
}

impl<S> Timeout<S> {
    /// Creates a new Timeout, given a timeout [`Duration`]
    pub const fn new(inner: S, timeout: Duration) -> Timeout<S> {
        Timeout { inner, timeout }
    }
}

impl<S, Req> Service<Req> for Timeout<S>
where
    S: Service<Req>,
    S::Error: Into<HttpServiceError>,
    S::Future: Send + 'static,
{
    type Response = S::Response;
    type Error = HttpServiceError;
    type Future = ResponseFuture<Result<Self::Response, Self::Error>>;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx).map_err(Into::into)
    }

    fn call(&mut self, req: Req) -> Self::Future {
        let resp = self.inner.call(req);

        let sleep = tokio::time::sleep(self.timeout);

        let fut = async move {
            tokio::pin!(sleep);
            tokio::pin!(resp);
            tokio::select! {
                _ = &mut sleep => {
                    Err(HttpServiceError::TimedOut)
                }
                result = &mut resp => {
                    result.map_err(Into::into)
                }
            }
        };

        Box::pin(fut)
    }
}

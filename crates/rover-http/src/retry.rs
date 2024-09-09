use std::{
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

use futures::{future::ready, Future, FutureExt};
use http::StatusCode;
use tower::{
    retry::{
        backoff::{Backoff, ExponentialBackoff, ExponentialBackoffMaker, MakeBackoff},
        Policy,
    },
    util::rng::HasherRng,
    Layer, Service,
};

use super::HttpServiceError;

#[derive(Clone, Debug, Default)]
pub struct RetryPolicy {
    count: usize,
    max: usize,
}

impl RetryPolicy {
    pub fn new(max: usize) -> RetryPolicy {
        RetryPolicy { count: 0, max }
    }
    pub fn increment(&mut self) {
        if self.count < self.max {
            self.count += 1
        }
    }
    pub fn can_retry(&self) -> bool {
        self.count < self.max
    }
}

impl Policy<http::Request<String>, http::Response<String>, HttpServiceError> for RetryPolicy {
    type Future = futures::future::Ready<()>;
    fn retry(
        &mut self,
        _: &mut http::Request<String>,
        result: &mut Result<http::Response<String>, HttpServiceError>,
    ) -> Option<Self::Future> {
        if self.can_retry() {
            self.increment();
            match result {
                Err(HttpServiceError::TimedOut(_))
                | Err(HttpServiceError::Connect(_))
                | Err(HttpServiceError::Incomplete(_)) => Some(ready(())),
                Err(_) => None,
                Ok(resp) => {
                    let status = resp.status();
                    if status.is_client_error()
                        || status.is_server_error()
                        || status.is_redirection()
                    {
                        if matches!(status, StatusCode::BAD_REQUEST) {
                            tracing::debug!("{}", resp.body());
                            None
                        } else {
                            Some(ready(()))
                        }
                    } else {
                        None
                    }
                }
            }
        } else {
            None
        }
    }

    fn clone_request(&mut self, _: &http::Request<String>) -> Option<http::Request<String>> {
        None
    }
}

#[derive(Clone, Debug)]
pub struct BackoffService<S> {
    backoff: ExponentialBackoff,
    service: S,
}

impl<S> BackoffService<S> {
    pub fn new(service: S, max_duration: Duration) -> BackoffService<S> {
        BackoffService {
            backoff: ExponentialBackoffMaker::new(
                Duration::from_millis(50),
                max_duration,
                0.99,
                HasherRng::default(),
            )
            .unwrap()
            .make_backoff(),
            service,
        }
    }
}

impl<T, S, Fut> Service<T> for BackoffService<S>
where
    S: Service<T, Future = Fut> + Clone + Send + 'static,
    T: Send + 'static,
    Fut: Future<Output = Result<S::Response, S::Error>> + Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.service.poll_ready(cx)
    }

    fn call(&mut self, req: T) -> Self::Future {
        let mut service = self.service.clone();
        let next_backoff = self.backoff.next_backoff();
        Box::pin(next_backoff.then(move |_| service.call(req)))
    }
}

pub struct BackoffLayer {
    max_duration: Duration,
}

impl BackoffLayer {
    pub fn new(max_duration: Duration) -> BackoffLayer {
        BackoffLayer { max_duration }
    }
}

impl<S> Layer<S> for BackoffLayer {
    type Service = BackoffService<S>;
    fn layer(&self, inner: S) -> Self::Service {
        BackoffService::new(inner, self.max_duration)
    }
}

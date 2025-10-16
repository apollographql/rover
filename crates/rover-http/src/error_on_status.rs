//! Provides Middleware that adds [`HttpServiceError::BadStatusCode`] as an error, given a range of [`StatusCode`]s

use std::{ops::Range, pin::Pin};

use futures::{Future, TryFutureExt};
use http::StatusCode;
use tower::{Layer, Service};

use crate::{HttpRequest, HttpResponse, HttpServiceError};

/// Describes a range of [`StatusCode`]s that may fail a request
#[derive(Clone)]
pub struct ErrorOnStatusCriteria {
    range: Range<StatusCode>,
}

impl ErrorOnStatusCriteria {
    /// Dictates whether a [`StatusCode`] matches the criteria
    pub fn matches(&self, status_code: &StatusCode) -> bool {
        self.range.contains(status_code)
    }
}

impl From<Range<StatusCode>> for ErrorOnStatusCriteria {
    fn from(value: Range<StatusCode>) -> Self {
        ErrorOnStatusCriteria { range: value }
    }
}

impl Default for ErrorOnStatusCriteria {
    fn default() -> Self {
        ErrorOnStatusCriteria::from(
            StatusCode::from_u16(400).unwrap()..StatusCode::from_u16(599).unwrap(),
        )
    }
}

/// [`Layer`] that attaches the [`ErrorOnStatus`] middleware to the [`Service`] stack
pub struct ErrorOnStatusLayer {
    criteria: ErrorOnStatusCriteria,
}

impl<S> Layer<S> for ErrorOnStatusLayer {
    type Service = ErrorOnStatus<S>;
    fn layer(&self, inner: S) -> Self::Service {
        ErrorOnStatus {
            criteria: self.criteria.clone(),
            inner,
        }
    }
}

/// Middleware that adds [`HttpServiceError::BadStatusCode`] as an error, given a range of [`StatusCode`]s
pub struct ErrorOnStatus<S> {
    criteria: ErrorOnStatusCriteria,
    inner: S,
}

impl<S> ErrorOnStatus<S> {
    /// Constructs a new [`ErrorOnStatus`]
    pub const fn new(criteria: ErrorOnStatusCriteria, inner: S) -> ErrorOnStatus<S> {
        ErrorOnStatus { criteria, inner }
    }
}

impl<S> Service<HttpRequest> for ErrorOnStatus<S>
where
    S: Service<HttpRequest, Response = HttpResponse, Error = HttpServiceError>,
    S::Future: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: HttpRequest) -> Self::Future {
        let criteria = self.criteria.clone();
        let fut = self.inner.call(req).and_then(|resp| async move {
            if criteria.matches(&resp.status()) {
                Err(HttpServiceError::BadStatusCode {
                    status_code: resp.status(),
                    response: resp.clone(),
                })
            } else {
                Ok(resp)
            }
        });
        Box::pin(fut)
    }
}

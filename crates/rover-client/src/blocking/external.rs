//! External authentication's GraphQL rejection classification and legacy retry eligibility.

use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
    time::{Duration, Instant},
};

use backon::BackoffBuilder;
use rover_graphql::is_credential_rejection;
use rover_http::{
    retry::is_retryable_status, BodyExt, HttpRequest, HttpResponse, HttpServiceError,
    ResponseStatus,
};
use rover_studio::service::external_auth::ExternalAuthError;
use serde::Deserialize;
use tower::{retry::Policy, Service};

use super::client::{get_source_error_type, GraphQLClient, REJECTED_CREDENTIAL_MESSAGE};

/// Recognizes authentication errors without allocating or deserializing the response's data.
#[derive(Deserialize)]
struct Errors {
    errors: Option<Vec<graphql_client::Error>>,
}

/// External mode intentionally uses Rover's shared GraphQL credential-rejection classifier
/// on every transport, including legacy calls; stored-credential classification is unchanged.
/// A body rejection is terminal. It is deliberately outside status-rejection handling so
/// a status rejection still gets its one opportunity to pick up an already-published token.
#[derive(Clone)]
pub(super) struct ExternalGraphQL<S> {
    inner: S,
}

impl<S> ExternalGraphQL<S> {
    pub(super) const fn new(inner: S) -> Self {
        Self { inner }
    }
}

impl<S> Service<HttpRequest> for ExternalGraphQL<S>
where
    S: Service<HttpRequest, Response = HttpResponse, Error = HttpServiceError>,
    S::Future: Send + 'static,
{
    type Response = HttpResponse;
    type Error = HttpServiceError;
    type Future = Pin<Box<dyn Future<Output = Result<HttpResponse, HttpServiceError>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, request: HttpRequest) -> Self::Future {
        let future = self.inner.call(request);
        Box::pin(async move {
            let response = future.await?;
            let bytes = response.body().clone().collect().await?.to_bytes();
            if serde_json::from_slice::<Errors>(&bytes)
                .ok()
                .and_then(|body| body.errors)
                .is_some_and(|errors| {
                    is_credential_rejection(&errors)
                        || errors
                            .iter()
                            .any(|error| error.message == REJECTED_CREDENTIAL_MESSAGE)
                })
            {
                return Err(ExternalAuthError::Rejected.into_http_error());
            }
            Ok(response)
        })
    }
}

/// The legacy request path retries fewer transport errors than the general HTTP policy.
/// Preserve that eligibility when it uses the shared external-authentication middleware.
#[derive(Debug)]
pub(super) struct LegacyRetryPolicy {
    backoff: backon::ExponentialBackoff,
    started: Instant,
    budget: Duration,
}

impl LegacyRetryPolicy {
    pub(super) fn new(budget: Duration) -> Self {
        Self {
            backoff: GraphQLClient::retry_backoff().build(),
            started: Instant::now(),
            budget,
        }
    }
}

impl Clone for LegacyRetryPolicy {
    fn clone(&self) -> Self {
        // Tower clones the policy at the start of each independent operation.
        Self::new(self.budget)
    }
}

impl Policy<HttpRequest, HttpResponse, HttpServiceError> for LegacyRetryPolicy {
    type Future = tokio::time::Sleep;

    fn retry(
        &mut self,
        request: &mut HttpRequest,
        result: &mut Result<HttpResponse, HttpServiceError>,
    ) -> Option<Self::Future> {
        // External authentication counts actual sends, including its one rejection resend,
        // so it owns RetryCount rather than this transient-failure policy.
        let eligible = match result {
            Ok(response) => is_retryable_status(response.status()),
            // Legacy response-body failures happen after the retrying send has completed.
            Err(HttpServiceError::TimedOut) => request
                .extensions()
                .get::<ResponseStatus>()
                .and_then(ResponseStatus::get)
                .is_none(),
            Err(HttpServiceError::Connect(_)) => true,
            Err(HttpServiceError::Request(source)) => source
                .downcast_ref::<reqwest::Error>()
                .and_then(|error| get_source_error_type::<hyper::Error>(error))
                .is_some_and(hyper::Error::is_incomplete_message),
            Err(_) => false,
        };
        if !eligible || self.started.elapsed() >= self.budget {
            return None;
        }
        let delay = self.backoff.next()?;
        Some(tokio::time::sleep(delay))
    }

    fn clone_request(&mut self, request: &HttpRequest) -> Option<HttpRequest> {
        Some(request.clone())
    }
}

#[cfg(test)]
mod tests {
    use http::Response;
    use http_body_util::Full;
    use rstest::rstest;
    use speculoos::prelude::*;

    use super::*;

    #[rstest]
    #[case::timeout(HttpServiceError::TimedOut, true)]
    #[case::connection(
        HttpServiceError::Connect(Box::new(std::io::Error::other("connection"))),
        true
    )]
    #[case::body(HttpServiceError::Body(Box::new(std::io::Error::other("body"))), false)]
    #[case::decode(
        HttpServiceError::Decode(Box::new(std::io::Error::other("decode"))),
        false
    )]
    #[case::closed(
        HttpServiceError::Closed(Box::new(std::io::Error::other("closed"))),
        false
    )]
    #[case::other_request(
        HttpServiceError::Request(Box::new(std::io::Error::other("request"))),
        false
    )]
    #[case::authentication(ExternalAuthError::Rejected.into_http_error(), false)]
    #[tokio::test]
    async fn legacy_transport_eligibility_is_not_broadened(
        #[case] error: HttpServiceError,
        #[case] expected_retry: bool,
    ) {
        let mut policy = LegacyRetryPolicy::new(Duration::from_secs(1));
        let mut request = http::Request::new(Full::default());
        let mut result = Err(error);

        let retried = policy.retry(&mut request, &mut result).is_some();

        assert_that!(retried).is_equal_to(expected_retry);
    }

    #[rstest]
    #[case::server_failure(503, true)]
    #[case::request_timeout(408, true)]
    #[case::too_early(425, true)]
    #[case::rate_limited(429, true)]
    #[case::unauthorized(401, false)]
    #[case::not_acceptable(406, false)]
    #[case::permission_denied(403, false)]
    #[case::bad_request(400, false)]
    #[case::success(200, false)]
    #[tokio::test]
    async fn legacy_status_eligibility_is_unchanged(
        #[case] status: u16,
        #[case] expected_retry: bool,
    ) {
        let mut policy = LegacyRetryPolicy::new(Duration::from_secs(1));
        let mut request = http::Request::new(Full::default());
        let mut result = Ok(Response::builder()
            .status(status)
            .body(Full::default())
            .unwrap());

        let retried = policy.retry(&mut request, &mut result).is_some();

        assert_that!(retried).is_equal_to(expected_retry);
    }

    #[rstest]
    #[tokio::test]
    async fn an_exhausted_legacy_budget_does_not_retry() {
        let mut policy = LegacyRetryPolicy::new(Duration::ZERO);
        let mut request = http::Request::new(Full::default());
        let mut result = Err(HttpServiceError::TimedOut);

        let retried = policy.retry(&mut request, &mut result).is_some();

        assert_that!(retried).is_equal_to(false);
    }
}

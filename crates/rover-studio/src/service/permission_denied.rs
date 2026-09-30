//! Provides middleware that recognizes when Apollo Studio has refused a request because the
//! caller isn't permitted to do this - distinct from [`super::rejected_credential`]'s "the
//! credential itself is bad" - so callers can report a permission problem instead of an opaque
//! HTTP status.

use std::{future::Future, pin::Pin};

use http::StatusCode;
use rover_http::{HttpRequest, HttpResponse, HttpServiceError};
use tower::{Layer, Service};

/// The status Apollo Studio uses to refuse an authenticated-but-not-permitted request.
///
/// Deliberately the mirror image of the `rejected_credential` module's own statuses: `401`/`406`
/// mean the credential itself is the problem; `403` means the credential authenticated fine but
/// isn't permitted to do this, which swapping the credential won't fix.
const PERMISSION_DENIED_STATUS: StatusCode = StatusCode::FORBIDDEN;

/// Apollo Studio refused a request because the caller isn't permitted to perform it.
///
/// This message is terse on purpose - the user-facing wording belongs to whoever renders the
/// error, matching [`super::rejected_credential::RejectedCredential`]'s own convention.
#[derive(thiserror::Error, Debug, Clone, Copy, PartialEq, Eq)]
#[error("the request authenticated fine but isn't permitted")]
pub struct PermissionDenied;

impl PermissionDenied {
    /// Wraps this so it can cross a boxed [`rover_http::HttpService`], whose error type is
    /// fixed. Mirrors
    /// [`super::rejected_credential::RejectedCredential::into_http_service_error`]:
    /// [`HttpServiceError::Unexpected`] is the carrier, matching that sibling's choice for
    /// consistency - not because it changes whether this gets retried. A 403 is never retried
    /// regardless of carrier (`rover_http::retry`'s retryable-status check excludes it, and
    /// `RetryPolicy` treats `Unexpected` as terminal either way).
    fn into_http_service_error(self) -> HttpServiceError {
        HttpServiceError::Unexpected(Box::new(self))
    }
}

/// Recovers a [`PermissionDenied`] from an error produced by [`PermissionDeniedLayer`].
pub fn permission_denied(err: &HttpServiceError) -> Option<PermissionDenied> {
    match err {
        HttpServiceError::Unexpected(source) => source.downcast_ref::<PermissionDenied>().copied(),
        _ => None,
    }
}

/// [`Layer`] that attaches the [`PermissionDeniedService`] middleware to the service stack.
///
/// Position relative to retry/timeout middleware doesn't affect whether a refusal gets replayed -
/// a 403 is already excluded from `rover_http::retry`'s retryable statuses, and the
/// [`HttpServiceError::Unexpected`] this produces is terminal to `RetryPolicy` regardless of
/// where it's raised. Placed next to [`super::rejected_credential::RejectedCredentialLayer`] for
/// consistency between the two closely related layers, not for that reason.
pub struct PermissionDeniedLayer;

impl<S: Clone> Layer<S> for PermissionDeniedLayer {
    type Service = PermissionDeniedService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        PermissionDeniedService { inner }
    }
}

/// Middleware that turns a permission refusal from Apollo Studio into a [`PermissionDenied`]
/// error.
#[derive(Clone)]
pub struct PermissionDeniedService<S: Clone> {
    inner: S,
}

impl<S> Service<HttpRequest> for PermissionDeniedService<S>
where
    S: Service<HttpRequest, Response = HttpResponse, Error = HttpServiceError> + Clone,
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
        let fut = self.inner.call(req);
        Box::pin(async move {
            let resp = fut.await?;
            if resp.status() == PERMISSION_DENIED_STATUS {
                tracing::debug!(status = ?resp.status(), "the registry refused the request for lack of permission");
                Err(PermissionDenied.into_http_service_error())
            } else {
                Ok(resp)
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use bytes::Bytes;
    use http::Method;
    use http_body_util::Full;
    use rover_http::test::MockHttpService;
    use rover_tower::test::{expect_poll_ready, MockCloneService};
    use rstest::rstest;
    use speculoos::prelude::*;
    use tower::ServiceExt;
    use url::Url;

    use super::*;

    /// Sends one request through the layer over a mock that answers with `status`. The error is
    /// boxed only to satisfy `clippy::result_large_err`.
    fn response_to_status(status: StatusCode) -> Result<HttpResponse, Box<HttpServiceError>> {
        let mut mock = MockHttpService::new();
        expect_poll_ready!(mock);
        mock.expect_call().returning(move |_| {
            futures::future::ready(Ok(http::Response::builder()
                .status(status)
                .body(Full::new(Bytes::default()))
                .unwrap()))
        });

        let service = PermissionDeniedLayer.layer(MockCloneService::new(mock));

        let req = http::Request::builder()
            .uri(Url::parse("https://example.com").unwrap().to_string())
            .method(Method::POST)
            .body(Full::default())
            .unwrap();

        futures::executor::block_on(service.oneshot(req)).map_err(Box::new)
    }

    #[test]
    fn a_forbidden_response_is_reported_as_permission_denied() {
        let err = response_to_status(StatusCode::FORBIDDEN)
            .expect_err("the refusal should have become an error");

        assert_that!(permission_denied(&err))
            .is_some()
            .is_equal_to(PermissionDenied);
    }

    // Includes `rejected_credential`'s own statuses (401/406) to document that the two layers
    // are deliberately disjoint - a credential rejection is never also reported as a permission
    // denial by this layer.
    #[rstest]
    #[case::ok(StatusCode::OK)]
    #[case::bad_request(StatusCode::BAD_REQUEST)]
    #[case::unauthorized(StatusCode::UNAUTHORIZED)]
    #[case::not_acceptable(StatusCode::NOT_ACCEPTABLE)]
    #[case::too_many_requests(StatusCode::TOO_MANY_REQUESTS)]
    #[case::internal_server_error(StatusCode::INTERNAL_SERVER_ERROR)]
    fn other_responses_pass_through_untouched(#[case] status: StatusCode) {
        let response = response_to_status(status);

        assert_that!(response.map(|resp| resp.status()))
            .is_ok()
            .is_equal_to(status);
    }

    #[test]
    fn unrelated_errors_are_not_read_as_permission_denials() {
        assert_that!(permission_denied(&HttpServiceError::TimedOut)).is_none();
        assert_that!(permission_denied(&HttpServiceError::Unexpected(Box::new(
            std::io::Error::other("something else")
        ))))
        .is_none();
    }
}

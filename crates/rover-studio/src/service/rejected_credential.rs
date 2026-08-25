//! Provides middleware that recognizes when Apollo Studio has rejected the credential a request
//! was made with, so callers can report a credential problem instead of an opaque HTTP status.

use std::{future::Future, pin::Pin};

use houston::Credential;
use http::StatusCode;
use rover_http::{HttpRequest, HttpResponse, HttpServiceError};
use tower::{Layer, Service};

/// Statuses Apollo Studio uses to reject a credential outright.
///
/// `403 Forbidden` is deliberately absent: it means the credential authenticated fine but isn't
/// permitted to do this, which fixing the key won't solve.
const CREDENTIAL_REJECTED_STATUSES: [StatusCode; 2] =
    [StatusCode::UNAUTHORIZED, StatusCode::NOT_ACCEPTABLE];

/// Apollo Studio rejected the credential a request was made with.
///
/// These messages are terse on purpose - the user-facing wording belongs to whoever renders the
/// error, in Rover's case `RoverClientError::MalformedKey` / `RoverClientError::InvalidKey`.
#[derive(thiserror::Error, Debug, Clone, Copy, PartialEq, Eq)]
pub enum RejectedCredential {
    /// The credential couldn't have been accepted in the first place, so its format is the
    /// thing to fix.
    #[error("the registry rejected a credential that isn't shaped like an API key")]
    MalformedKey,

    /// The credential is shaped correctly but was still refused: revoked, expired, or unknown.
    #[error("the registry did not accept the credential")]
    InvalidKey,
}

impl RejectedCredential {
    fn for_credential(credential: &Credential) -> RejectedCredential {
        if credential.has_malformed_api_key() {
            RejectedCredential::MalformedKey
        } else {
            RejectedCredential::InvalidKey
        }
    }

    /// Wraps this so it can cross a boxed [`rover_http::HttpService`], whose error type is fixed.
    ///
    /// [`HttpServiceError::Unexpected`] is the carrier on purpose: it's the one variant
    /// [`rover_http::retry::RetryPolicy`] treats as terminal, and routing this through
    /// [`HttpServiceError::Request`] would make the retry policy replay a refused credential.
    fn into_http_service_error(self) -> HttpServiceError {
        HttpServiceError::Unexpected(Box::new(self))
    }
}

/// Recovers a [`RejectedCredential`] from an error produced by [`RejectedCredentialLayer`].
pub fn rejected_credential(err: &HttpServiceError) -> Option<RejectedCredential> {
    match err {
        HttpServiceError::Unexpected(source) => {
            source.downcast_ref::<RejectedCredential>().copied()
        }
        _ => None,
    }
}

/// [`Layer`] that attaches the [`RejectedCredentialService`] middleware to the service stack.
///
/// Place this **above** the retry middleware: below it, a credential the registry has already
/// refused would be handed to the retry policy and sent again.
pub struct RejectedCredentialLayer {
    credential: Credential,
}

impl RejectedCredentialLayer {
    /// Constructs a new [`RejectedCredentialLayer`] over the credential requests are
    /// authenticated with, consulted only to decide whether a rejection is the key's format.
    pub const fn new(credential: Credential) -> RejectedCredentialLayer {
        RejectedCredentialLayer { credential }
    }
}

impl<S: Clone> Layer<S> for RejectedCredentialLayer {
    type Service = RejectedCredentialService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        RejectedCredentialService {
            rejection: RejectedCredential::for_credential(&self.credential),
            inner,
        }
    }
}

/// Middleware that turns a credential rejection from Apollo Studio into a
/// [`RejectedCredential`] error.
#[derive(Clone)]
pub struct RejectedCredentialService<S: Clone> {
    rejection: RejectedCredential,
    inner: S,
}

impl<S> Service<HttpRequest> for RejectedCredentialService<S>
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
        let rejection = self.rejection;
        let fut = self.inner.call(req);
        Box::pin(async move {
            let resp = fut.await?;
            if CREDENTIAL_REJECTED_STATUSES.contains(&resp.status()) {
                tracing::debug!(status = ?resp.status(), "the registry rejected our credential");
                Err(rejection.into_http_service_error())
            } else {
                Ok(resp)
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use bytes::Bytes;
    use houston::CredentialOrigin;
    use http::Method;
    use http_body_util::Full;
    use rover_http::test::MockHttpService;
    use rover_tower::test::{expect_poll_ready, MockCloneService};
    use rstest::rstest;
    use speculoos::prelude::*;
    use tower::ServiceExt;
    use url::Url;

    use super::*;

    fn credential(api_key: &str) -> Credential {
        Credential {
            api_key: api_key.to_string(),
            origin: CredentialOrigin::EnvVar,
            expires_at: None,
        }
    }

    /// Sends one request through the layer over a mock that answers with `status`. The error is
    /// boxed only to satisfy `clippy::result_large_err`.
    fn response_to_status(
        api_key: &str,
        status: StatusCode,
    ) -> Result<HttpResponse, Box<HttpServiceError>> {
        let mut mock = MockHttpService::new();
        expect_poll_ready!(mock);
        mock.expect_call().returning(move |_| {
            futures::future::ready(Ok(http::Response::builder()
                .status(status)
                .body(Full::new(Bytes::default()))
                .unwrap()))
        });

        let service =
            RejectedCredentialLayer::new(credential(api_key)).layer(MockCloneService::new(mock));

        let req = http::Request::builder()
            .uri(Url::from_str("https://example.com").unwrap().to_string())
            .method(Method::POST)
            .body(Full::default())
            .unwrap();

        futures::executor::block_on(service.oneshot(req)).map_err(Box::new)
    }

    #[rstest]
    #[case::unauthorized(StatusCode::UNAUTHORIZED)]
    #[case::not_acceptable(StatusCode::NOT_ACCEPTABLE)]
    fn a_rejection_of_a_well_formed_key_is_reported_as_invalid(#[case] status: StatusCode) {
        let err = response_to_status("user:my-username:secretkey", status)
            .expect_err("the rejection should have become an error");

        assert_that!(rejected_credential(&err))
            .is_some()
            .is_equal_to(RejectedCredential::InvalidKey);
    }

    #[rstest]
    #[case::unauthorized(StatusCode::UNAUTHORIZED)]
    #[case::not_acceptable(StatusCode::NOT_ACCEPTABLE)]
    fn a_rejection_of_a_bad_shaped_key_is_reported_as_malformed(#[case] status: StatusCode) {
        let err = response_to_status("not-a-real-key", status)
            .expect_err("the rejection should have become an error");

        assert_that!(rejected_credential(&err))
            .is_some()
            .is_equal_to(RejectedCredential::MalformedKey);
    }

    #[test]
    fn a_forbidden_response_is_not_treated_as_a_credential_rejection() {
        let response = response_to_status("not-a-real-key", StatusCode::FORBIDDEN);

        assert_that!(response.map(|resp| resp.status()))
            .is_ok()
            .is_equal_to(StatusCode::FORBIDDEN);
    }

    #[rstest]
    #[case::ok(StatusCode::OK)]
    #[case::bad_request(StatusCode::BAD_REQUEST)]
    #[case::too_many_requests(StatusCode::TOO_MANY_REQUESTS)]
    #[case::internal_server_error(StatusCode::INTERNAL_SERVER_ERROR)]
    fn other_responses_pass_through_untouched(#[case] status: StatusCode) {
        let response = response_to_status("not-a-real-key", status);

        assert_that!(response.map(|resp| resp.status()))
            .is_ok()
            .is_equal_to(status);
    }

    #[test]
    fn unrelated_errors_are_not_read_as_credential_rejections() {
        assert_that!(rejected_credential(&HttpServiceError::TimedOut)).is_none();
        assert_that!(rejected_credential(&HttpServiceError::Unexpected(
            Box::new(std::io::Error::other("something else"))
        )))
        .is_none();
    }
}

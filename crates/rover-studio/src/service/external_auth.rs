//! Application-owned bearer authentication for the configured Platform API endpoint.
//!
//! Place [`ExternalAuthOperationLayer`] outside retry middleware and [`ExternalAuthLayer`]
//! inside it, before the per-attempt timeout and trace layers. Keep the timeout inside
//! authentication so it bounds each network send, not source reads or a pair of sends.
//! The operation layer shares only an attempt counter and a resend allowance across
//! request clones, never a cached credential.
//! The HTTP transport must not follow redirects: a redirect is not another Platform API attempt.

use std::{
    future::{ready, Future, Ready},
    pin::Pin,
    sync::{
        atomic::{AtomicBool, AtomicU32, Ordering},
        Arc,
    },
    task::{Context, Poll},
};

use http::{header::AUTHORIZATION, HeaderValue, StatusCode, Uri};
use rover_auth::external_token::{ExternalAccessToken, ExternalTokenError};
use rover_http::{retry::RetryCount, HttpRequest, HttpResponse, HttpServiceError};
use rover_tower::{service::replace_ready_service, ResponseFuture};
use tower::{
    retry::{Policy, Retry},
    Layer, Service, ServiceExt,
};

/// Terminal external-authentication failures, distinct from transient transport failures.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ExternalAuthError {
    /// The supplying application has not published a usable token.
    #[error(transparent)]
    Source(#[from] ExternalTokenError),
    /// Authentication failed, including an unchanged token after a status rejection.
    #[error(
        "The application-supplied credential was rejected. Renew it in the supplying application, then try again."
    )]
    Rejected,
    /// The authenticated account is not allowed to perform the operation.
    #[error("The application-supplied credential does not have permission for this operation.")]
    PermissionDenied,
}

impl From<ExternalAuthError> for HttpServiceError {
    /// Carries a terminal failure through an HTTP service without making it retryable.
    fn from(error: ExternalAuthError) -> Self {
        Self::Unexpected(Box::new(error))
    }
}

impl<'a> TryFrom<&'a HttpServiceError> for &'a ExternalAuthError {
    type Error = &'a HttpServiceError;

    /// Recovers a structured external-authentication failure from a boxed HTTP service.
    fn try_from(error: &'a HttpServiceError) -> Result<Self, Self::Error> {
        match error {
            HttpServiceError::Unexpected(source) => source.downcast_ref().ok_or(error),
            _ => Err(error),
        }
    }
}

#[derive(Clone)]
struct AuthenticationRequest {
    request: HttpRequest,
    rejected_token: Option<ExternalAccessToken>,
}

struct AuthenticationResponse {
    response: HttpResponse,
    token: ExternalAccessToken,
}

/// Clones share one authentication resend and send counter for a logical operation.
#[derive(Clone, Default)]
struct AuthenticationRetryPolicy {
    resend_used: Arc<AtomicBool>,
    attempts: Arc<AtomicU32>,
}

impl Policy<AuthenticationRequest, AuthenticationResponse, HttpServiceError>
    for AuthenticationRetryPolicy
{
    type Future = Ready<()>;

    fn retry(
        &mut self,
        request: &mut AuthenticationRequest,
        result: &mut Result<AuthenticationResponse, HttpServiceError>,
    ) -> Option<Self::Future> {
        let response = result.as_ref().ok()?;
        if !matches!(
            response.response.status(),
            StatusCode::UNAUTHORIZED | StatusCode::NOT_ACCEPTABLE
        ) || self.resend_used.swap(true, Ordering::Relaxed)
        {
            return None;
        }

        request.rejected_token = Some(response.token.clone());
        Some(ready(()))
    }

    fn clone_request(&mut self, request: &AuthenticationRequest) -> Option<AuthenticationRequest> {
        Some(request.clone())
    }
}

/// Applies its token and the shared attempt count immediately before sending.
#[derive(Clone)]
struct Authorize<S> {
    inner: S,
    token: ExternalAccessToken,
    policy: AuthenticationRetryPolicy,
}

impl<S> Service<HttpRequest> for Authorize<S>
where
    S: Service<HttpRequest, Response = HttpResponse, Error = HttpServiceError>,
    S::Future: Send + 'static,
{
    type Response = HttpResponse;
    type Error = HttpServiceError;
    type Future = ResponseFuture<Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut request: HttpRequest) -> Self::Future {
        let mut value = match HeaderValue::from_str(&format!("Bearer {}", self.token.secret())) {
            Ok(value) => value,
            Err(error) => {
                return Box::pin(std::future::ready(Err(http::Error::from(error).into())));
            }
        };
        value.set_sensitive(true);
        request.headers_mut().remove("x-api-key");
        request.headers_mut().insert(AUTHORIZATION, value);
        request.extensions_mut().insert(RetryCount(
            self.policy.attempts.fetch_add(1, Ordering::Relaxed),
        ));
        Box::pin(self.inner.call(request))
    }
}

/// Reads and checks the token before each authentication attempt, outside the network timeout.
struct Authenticate<S, T> {
    inner: S,
    source: T,
    policy: AuthenticationRetryPolicy,
    // Only the initial send inherits the caller's readiness reservation.
    network_ready: bool,
}

impl<S: Clone, T: Clone> Clone for Authenticate<S, T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            source: self.source.clone(),
            policy: self.policy.clone(),
            network_ready: false,
        }
    }
}

impl<S, T> Service<AuthenticationRequest> for Authenticate<S, T>
where
    S: Service<HttpRequest, Response = HttpResponse, Error = HttpServiceError>
        + Clone
        + Send
        + 'static,
    S::Future: Send,
    T: Service<(), Response = ExternalAccessToken, Error = ExternalTokenError>
        + Clone
        + Send
        + 'static,
    T::Future: Send,
{
    type Response = AuthenticationResponse;
    type Error = HttpServiceError;
    type Future = ResponseFuture<Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        // Check replacement credentials before reserving another network send.
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, attempt: AuthenticationRequest) -> Self::Future {
        let network_ready = std::mem::take(&mut self.network_ready);
        let inner = replace_ready_service(&mut self.inner);
        let source = self.source.clone();
        let policy = self.policy.clone();

        Box::pin(async move {
            let token = source.oneshot(()).await.map_err(ExternalAuthError::from)?;
            if attempt
                .rejected_token
                .as_ref()
                .is_some_and(|rejected| rejected == &token)
            {
                return Err(ExternalAuthError::Rejected.into());
            }

            let mut inner = Authorize {
                inner,
                token,
                policy,
            };
            let response = if network_ready {
                inner.call(attempt.request).await?
            } else {
                inner.ready().await?.call(attempt.request).await?
            };
            Ok(AuthenticationResponse {
                response,
                token: inner.token,
            })
        })
    }
}

/// Starts a fresh resend allowance and trace-attempt counter for each logical operation.
///
/// Put this outside retry middleware so every retry shares the same operation state.
#[derive(Clone, Copy, Debug, Default)]
pub struct ExternalAuthOperationLayer;

impl<S> Layer<S> for ExternalAuthOperationLayer {
    type Service = ExternalAuthOperation<S>;

    fn layer(&self, inner: S) -> Self::Service {
        ExternalAuthOperation { inner }
    }
}

/// Operation boundary for [`ExternalAuthLayer`].
#[derive(Clone)]
pub struct ExternalAuthOperation<S> {
    inner: S,
}

impl<S: Service<HttpRequest>> Service<HttpRequest> for ExternalAuthOperation<S> {
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut request: HttpRequest) -> Self::Future {
        request
            .extensions_mut()
            .insert(AuthenticationRetryPolicy::default());
        self.inner.call(request)
    }
}

/// Reads the token immediately before each attempt to the exact configured endpoint.
///
/// The token source is a service so tests and other callers need not use a filesystem.
/// Pair this with [`ExternalAuthOperationLayer`] outside any transient retry middleware.
#[derive(Clone, Debug)]
pub struct ExternalAuthLayer<T> {
    source: T,
    endpoint: Uri,
}

impl<T> ExternalAuthLayer<T> {
    /// Constructs an inert layer. Neither construction nor readiness reads the source.
    pub const fn new(source: T, endpoint: Uri) -> Self {
        Self { source, endpoint }
    }
}

impl<S, T: Clone> Layer<S> for ExternalAuthLayer<T> {
    type Service = ExternalAuth<S, T>;

    fn layer(&self, inner: S) -> Self::Service {
        ExternalAuth {
            inner,
            source: self.source.clone(),
            endpoint: self.endpoint.clone(),
        }
    }
}

/// Per-attempt authentication and the single immediate resend after a status rejection.
#[derive(Clone)]
pub struct ExternalAuth<S, T> {
    inner: S,
    source: T,
    endpoint: Uri,
}

impl<S, T> ExternalAuth<S, T> {
    fn classify(response: HttpResponse) -> Result<HttpResponse, ExternalAuthError> {
        match response.status() {
            StatusCode::UNAUTHORIZED | StatusCode::NOT_ACCEPTABLE => {
                Err(ExternalAuthError::Rejected)
            }
            StatusCode::FORBIDDEN => Err(ExternalAuthError::PermissionDenied),
            _ => Ok(response),
        }
    }
}

impl<S, T> Service<HttpRequest> for ExternalAuth<S, T>
where
    S: Service<HttpRequest, Response = HttpResponse, Error = HttpServiceError>
        + Clone
        + Send
        + 'static,
    S::Future: Send,
    T: Service<(), Response = ExternalAccessToken, Error = ExternalTokenError>
        + Clone
        + Send
        + 'static,
    T::Future: Send,
{
    type Response = HttpResponse;
    type Error = HttpServiceError;
    type Future = Pin<Box<dyn Future<Output = Result<HttpResponse, HttpServiceError>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, request: HttpRequest) -> Self::Future {
        if request.uri() != &self.endpoint {
            return Box::pin(self.inner.call(request));
        }

        let policy = request
            .extensions()
            .get::<AuthenticationRetryPolicy>()
            .cloned()
            .unwrap_or_default();
        let mut inner = Retry::new(
            policy.clone(),
            Authenticate {
                inner: replace_ready_service(&mut self.inner),
                source: self.source.clone(),
                policy,
                network_ready: true,
            },
        );
        let response = inner.call(AuthenticationRequest {
            request,
            rejected_token: None,
        });

        Box::pin(async move {
            let AuthenticationResponse { response, .. } = response.await?;
            Self::classify(response).map_err(HttpServiceError::from)
        })
    }
}

/// Mock token sources for external authentication consumers and tests.
#[cfg(any(test, feature = "testing"))]
pub mod mock {
    use rover_auth::external_token::{ExternalAccessToken, ExternalTokenError};

    /// Request accepted by the authentication middleware's token source.
    pub type TokenRequest = ();

    rover_tower::mock_service!(Token, TokenRequest, ExternalAccessToken, ExternalTokenError);
}

#[cfg(test)]
mod tests {
    use std::{
        collections::{HashMap, HashSet, VecDeque},
        sync::Mutex,
        time::Duration,
    };

    use bytes::Bytes;
    use futures::{future::ready, stream, FutureExt, StreamExt};
    use http::{HeaderMap, HeaderName, Method, Request, Response};
    use http_body_util::{BodyExt, Full};
    use rover_auth::external_token::{ExternalTokenFailure, ExternalTokenOrigin};
    use rover_http::{
        retry::RetryPolicy,
        test::MockHttpService,
        timeout::TimeoutLayer,
        trace::{new_trace_id, TraceparentLayer},
    };
    use rover_tower::{expect_poll_ready, test::MockCloneService};
    use rstest::{fixture, rstest};
    use speculoos::prelude::*;
    use tower::{retry::RetryLayer, ServiceBuilder};

    use super::{mock::MockTokenService, *};

    #[derive(Debug, PartialEq, Eq)]
    struct Attempt {
        method: Method,
        uri: Uri,
        authorization: Option<String>,
        sensitive: bool,
        api_key: Option<String>,
        body: Bytes,
        retries: String,
    }

    impl From<&HttpRequest> for Attempt {
        fn from(request: &HttpRequest) -> Self {
            Self {
                method: request.method().clone(),
                uri: request.uri().clone(),
                authorization: request
                    .headers()
                    .get(AUTHORIZATION)
                    .map(|value| value.to_str().unwrap().to_owned()),
                sensitive: request
                    .headers()
                    .get(AUTHORIZATION)
                    .is_some_and(HeaderValue::is_sensitive),
                api_key: request
                    .headers()
                    .get("x-api-key")
                    .map(|value| value.to_str().unwrap().to_owned()),
                body: request
                    .body()
                    .clone()
                    .collect()
                    .now_or_never()
                    .unwrap()
                    .unwrap()
                    .to_bytes(),
                retries: request.headers()["tracestate"].to_str().unwrap().to_owned(),
            }
        }
    }

    #[fixture]
    fn endpoint() -> Uri {
        "https://platform.example/api/graphql".parse().unwrap()
    }

    #[fixture]
    fn source_failure() -> ExternalTokenError {
        ExternalTokenError {
            origin: ExternalTokenOrigin::Environment,
            path: "application/access-token".into(),
            kind: ExternalTokenFailure::Missing,
        }
    }

    #[rstest]
    #[case::unauthorized(StatusCode::UNAUTHORIZED)]
    #[case::not_acceptable(StatusCode::NOT_ACCEPTABLE)]
    fn authentication_retry_policy_clones_share_one_resend(
        endpoint: Uri,
        #[case] status: StatusCode,
    ) {
        let mut policy = AuthenticationRetryPolicy::default();
        let mut cloned_policy = policy.clone();
        let token = ExternalAccessToken::parse(b"rejected-token").unwrap();
        let mut request = AuthenticationRequest {
            request: Request::builder()
                .uri(endpoint)
                .body(Full::default())
                .unwrap(),
            rejected_token: None,
        };
        let mut result = Ok(AuthenticationResponse {
            response: Response::builder()
                .status(status)
                .body(Full::default())
                .unwrap(),
            token: token.clone(),
        });

        let first = policy
            .retry(&mut request, &mut result)
            .map(FutureExt::now_or_never);
        let second = cloned_policy
            .retry(&mut request, &mut result)
            .map(FutureExt::now_or_never);
        let replay = policy.clone_request(&request);

        assert_that!(first).is_some().is_some().is_equal_to(());
        assert_that!(second).is_none();
        assert_that!(request.rejected_token.as_ref())
            .is_some()
            .is_equal_to(&token);
        assert_that!(replay.map(|request| request.rejected_token))
            .is_some()
            .is_some()
            .is_equal_to(token.clone());
        assert_that!(result.map(|response| (response.response.status(), response.token)))
            .is_ok()
            .is_equal_to((status, token));
        assert_that!(policy.attempts.load(Ordering::Relaxed)).is_equal_to(0);
    }

    #[rstest]
    #[case::source_failure(None, false)]
    #[case::unchanged_token(Some("A"), false)]
    #[case::network_readiness_failure(Some("B"), true)]
    #[tokio::test]
    async fn authentication_retry_checks_the_source_before_network_readiness(
        endpoint: Uri,
        source_failure: ExternalTokenError,
        #[case] replacement: Option<&str>,
        #[case] reaches_network_readiness: bool,
    ) {
        let reads = Arc::new(AtomicU32::new(0));
        let counted_reads = reads.clone();
        let mut tokens = VecDeque::from([
            Ok(ExternalAccessToken::parse(b"A").unwrap()),
            replacement
                .map(|token| ExternalAccessToken::parse(token.as_bytes()).unwrap())
                .ok_or_else(|| source_failure.clone()),
        ]);
        let mut source = MockTokenService::new();
        source
            .expect_poll_ready()
            .returning(|_| Poll::Ready(Ok(())));
        source.expect_call().times(1..=2).returning(move |_| {
            counted_reads.fetch_add(1, Ordering::Relaxed);
            ready(tokens.pop_front().unwrap())
        });
        let polls = Arc::new(AtomicU32::new(0));
        let counted_polls = polls.clone();
        let mut http = MockHttpService::new();
        http.expect_poll_ready().times(1..=2).returning(move |_| {
            if counted_polls.fetch_add(1, Ordering::Relaxed) == 0 {
                Poll::Ready(Ok(()))
            } else {
                Poll::Ready(Err(HttpServiceError::TimedOut))
            }
        });
        http.expect_call().once().return_once(|_| {
            ready(Ok(Response::builder()
                .status(StatusCode::UNAUTHORIZED)
                .body(Full::default())
                .unwrap()))
        });
        let service = ExternalAuthLayer::new(MockCloneService::new(source), endpoint.clone())
            .layer(MockCloneService::new(http));

        let result = service
            .oneshot(
                Request::builder()
                    .uri(endpoint)
                    .body(Full::default())
                    .unwrap(),
            )
            .await;

        if reaches_network_readiness {
            assert_that!(result.map_err(|error| matches!(error, HttpServiceError::TimedOut)))
                .is_err()
                .is_equal_to(true);
        } else {
            let expected = match replacement {
                Some(_) => ExternalAuthError::Rejected,
                None => ExternalAuthError::Source(source_failure),
            };
            assert_that!(result.as_ref().map_err(<&ExternalAuthError>::try_from))
                .is_err()
                .is_ok()
                .is_equal_to(&expected);
        }
        assert_that!(reads.load(Ordering::Relaxed)).is_equal_to(2);
        assert_that!(polls.load(Ordering::Relaxed))
            .is_equal_to(1 + u32::from(reaches_network_readiness));
    }

    #[rstest]
    #[tokio::test]
    async fn authorization_service_uses_its_token_and_shared_attempt_count(endpoint: Uri) {
        let attempts = Arc::new(Mutex::new(Vec::new()));
        let captured = attempts.clone();
        let mut http = MockHttpService::new();
        expect_poll_ready!(http, 2);
        http.expect_call().times(2).returning(move |request| {
            captured.lock().unwrap().push((
                request.method().clone(),
                request.uri().clone(),
                request.headers().clone(),
                request.headers()[AUTHORIZATION].is_sensitive(),
                RetryCount::of(&request).0,
                request
                    .body()
                    .clone()
                    .collect()
                    .now_or_never()
                    .unwrap()
                    .unwrap()
                    .to_bytes(),
            ));
            ready(Ok(Response::builder()
                .status(StatusCode::NO_CONTENT)
                .body(Full::default())
                .unwrap()))
        });
        let policy = AuthenticationRetryPolicy {
            resend_used: Arc::new(AtomicBool::new(true)),
            attempts: Arc::new(AtomicU32::new(7)),
        };
        let service = Authorize {
            inner: MockCloneService::new(http),
            token: ExternalAccessToken::parse(b"first-token").unwrap(),
            policy: policy.clone(),
        };
        let mut request = Request::builder()
            .method(Method::POST)
            .uri(endpoint.clone())
            .header(AUTHORIZATION, "old-authorization")
            .header(AUTHORIZATION, "another-old-authorization")
            .header("x-api-key", "old-api-key")
            .header("x-api-key", "another-old-api-key")
            .header("x-preserved-header", "preserved")
            .body(Full::new(Bytes::from_static(b"original request body")))
            .unwrap();
        request.extensions_mut().insert(RetryCount(99));

        let first = service
            .clone()
            .oneshot(request.clone())
            .await
            .map(|response| response.status());
        let second = Authorize {
            token: ExternalAccessToken::parse(b"second-token").unwrap(),
            ..service
        }
        .oneshot(request)
        .await
        .map(|response| response.status());
        let expected = [("Bearer first-token", 7), ("Bearer second-token", 8)]
            .map(|(authorization, attempt)| {
                (
                    Method::POST,
                    endpoint.clone(),
                    HeaderMap::from_iter([
                        (AUTHORIZATION, HeaderValue::from_static(authorization)),
                        (
                            HeaderName::from_static("x-preserved-header"),
                            HeaderValue::from_static("preserved"),
                        ),
                    ]),
                    true,
                    attempt,
                    Bytes::from_static(b"original request body"),
                )
            })
            .to_vec();

        assert_that!(first)
            .is_ok()
            .is_equal_to(StatusCode::NO_CONTENT);
        assert_that!(second)
            .is_ok()
            .is_equal_to(StatusCode::NO_CONTENT);
        assert_that!(*attempts.lock().unwrap()).is_equal_to(expected);
        assert_that!(policy.attempts.load(Ordering::Relaxed)).is_equal_to(9);
        assert_that!(policy.resend_used.load(Ordering::Relaxed)).is_equal_to(true);
    }

    #[rstest]
    #[case::pending(Poll::Pending, None)]
    #[case::failure(
        Poll::Ready(Err(HttpServiceError::TimedOut)),
        Some("Request timed out")
    )]
    fn authorization_service_delegates_readiness_without_counting_an_attempt(
        #[case] readiness: Poll<Result<(), HttpServiceError>>,
        #[case] expected_error: Option<&str>,
    ) {
        let mut http = MockHttpService::new();
        http.expect_poll_ready()
            .once()
            .return_once(move |_| readiness);
        let policy = AuthenticationRetryPolicy::default();
        let mut service = Authorize {
            inner: http,
            token: ExternalAccessToken::parse(b"private-token").unwrap(),
            policy: policy.clone(),
        };

        let result = service
            .ready()
            .now_or_never()
            .map(|result| result.map(|_| ()).map_err(|error| error.to_string()));

        match expected_error {
            Some(expected) => {
                assert_that!(result)
                    .is_some()
                    .is_err()
                    .is_equal_to(expected.to_string());
            }
            None => {
                assert_that!(result).is_none();
            }
        }
        assert_that!(policy.attempts.load(Ordering::Relaxed)).is_equal_to(0);
        assert_that!(policy.resend_used.load(Ordering::Relaxed)).is_equal_to(false);
    }

    struct Plan {
        source: Vec<Result<ExternalAccessToken, ExternalTokenError>>,
        statuses: Vec<StatusCode>,
    }

    struct Outcome {
        result: Result<StatusCode, ExternalAuthError>,
        attempts: Vec<Attempt>,
        traceparents: Vec<String>,
    }

    impl Plan {
        fn new(tokens: &[&str], statuses: &[u16]) -> Self {
            Self {
                source: tokens
                    .iter()
                    .map(|token| Ok(ExternalAccessToken::parse(token.as_bytes()).unwrap()))
                    .collect(),
                statuses: statuses
                    .iter()
                    .map(|status| StatusCode::from_u16(*status).unwrap())
                    .collect(),
            }
        }

        async fn run(self, endpoint: Uri, request_uri: Uri) -> Outcome {
            let mut tokens = VecDeque::from(self.source);
            let mut source = MockTokenService::new();
            source
                .expect_poll_ready()
                .returning(|_| Poll::Ready(Ok(())));
            source
                .expect_call()
                .times(tokens.len())
                .returning(move |_| ready(tokens.pop_front().unwrap()));

            let attempts = Arc::new(Mutex::new(Vec::new()));
            let captured = attempts.clone();
            let mut statuses = VecDeque::from(self.statuses);
            let mut http = MockHttpService::new();
            http.expect_poll_ready().returning(|_| Poll::Ready(Ok(())));
            http.expect_call()
                .times(statuses.len())
                .returning(move |request| {
                    captured.lock().unwrap().push((
                        Attempt::from(&request),
                        request.headers()["traceparent"]
                            .to_str()
                            .unwrap()
                            .to_owned(),
                    ));
                    ready(Ok(Response::builder()
                        .status(statuses.pop_front().unwrap())
                        .body(Full::new(Bytes::from_static(b"response body")))
                        .unwrap()))
                });

            let service = ServiceBuilder::new()
                .layer(ExternalAuthOperationLayer)
                // Authentication must sit between retry and timeout: a source read is
                // not a network attempt, and each of the two possible sends is bounded.
                .layer(RetryLayer::new(RetryPolicy::new(Duration::from_secs(5))))
                .layer(ExternalAuthLayer::new(
                    MockCloneService::new(source),
                    endpoint,
                ))
                .layer(TimeoutLayer::new(Duration::from_secs(1)))
                .layer(TraceparentLayer::new(new_trace_id()))
                .service(MockCloneService::new(http));
            let request = Request::builder()
                .method(Method::POST)
                .uri(request_uri)
                .header("x-api-key", "must-be-removed")
                .header(AUTHORIZATION, "must-be-replaced")
                .body(Full::new(Bytes::from_static(b"original request body")))
                .unwrap();
            let result = service
                .oneshot(request)
                .await
                .map(|response| response.status())
                .map_err(|error| <&ExternalAuthError>::try_from(&error).unwrap().clone());
            let (attempts, traceparents) = std::mem::take(&mut *attempts.lock().unwrap())
                .into_iter()
                .unzip();
            Outcome {
                result,
                attempts,
                traceparents,
            }
        }
    }

    #[rstest]
    #[case::first_attempt(&["A"], &[200], Ok(StatusCode::OK), &["A"])]
    #[case::unchanged_unauthorized(&["A", "A"], &[401], Err(ExternalAuthError::Rejected), &["A"])]
    #[case::unchanged_not_acceptable(&["A", "A"], &[406], Err(ExternalAuthError::Rejected), &["A"])]
    #[case::rotated_unauthorized(&["A", "B"], &[401, 200], Ok(StatusCode::OK), &["A", "B"])]
    #[case::rotated_not_acceptable(&["A", "B"], &[406, 200], Ok(StatusCode::OK), &["A", "B"])]
    #[case::second_rejection(&["A", "B"], &[401, 406], Err(ExternalAuthError::Rejected), &["A", "B"])]
    #[case::permission_denied(&["A"], &[403], Err(ExternalAuthError::PermissionDenied), &["A"])]
    #[case::resend_permission_denied(&["A", "B"], &[401, 403], Err(ExternalAuthError::PermissionDenied), &["A", "B"])]
    #[case::transient_rotation(&["A", "B"], &[503, 200], Ok(StatusCode::OK), &["A", "B"])]
    #[case::rejection_after_retry(&["A", "B", "C"], &[503, 401, 200], Ok(StatusCode::OK), &["A", "B", "C"])]
    #[case::retry_after_resend(&["A", "B", "C"], &[401, 503, 200], Ok(StatusCode::OK), &["A", "B", "C"])]
    #[case::resend_allowance_survives_retry(&["A", "B", "C"], &[401, 503, 401], Err(ExternalAuthError::Rejected), &["A", "B", "C"])]
    #[tokio::test(start_paused = true)]
    async fn authenticates_each_attempt_without_expanding_the_resend_allowance(
        endpoint: Uri,
        #[case] tokens: &[&str],
        #[case] statuses: &[u16],
        #[case] expected: Result<StatusCode, ExternalAuthError>,
        #[case] sent_tokens: &[&str],
    ) {
        let outcome = Plan::new(tokens, statuses)
            .run(endpoint.clone(), endpoint.clone())
            .await;
        let expected_attempts = sent_tokens
            .iter()
            .enumerate()
            .map(|(retry, token)| Attempt {
                method: Method::POST,
                uri: endpoint.clone(),
                authorization: Some(format!("Bearer {token}")),
                sensitive: true,
                api_key: None,
                body: Bytes::from_static(b"original request body"),
                retries: format!("rover=retries:{retry}"),
            })
            .collect::<Vec<_>>();

        match expected {
            Ok(expected) => {
                assert_that!(outcome.result).is_ok().is_equal_to(expected);
            }
            Err(expected) => {
                assert_that!(outcome.result).is_err().is_equal_to(expected);
            }
        }
        assert_that!(outcome.attempts).is_equal_to(expected_attempts);
        let trace_ids = outcome
            .traceparents
            .iter()
            .map(|parent| parent.split('-').nth(1).unwrap())
            .collect::<HashSet<_>>();
        let span_ids = outcome
            .traceparents
            .iter()
            .map(|parent| parent.split('-').nth(2).unwrap())
            .collect::<HashSet<_>>();
        assert_that!(trace_ids.len()).is_equal_to(1);
        assert_that!(span_ids.len()).is_equal_to(sent_tokens.len());
    }

    #[rstest]
    #[case::before_first_send(None)]
    #[case::after_rejection(Some(401))]
    #[case::after_transient_failure(Some(503))]
    #[tokio::test]
    async fn a_source_failure_is_terminal_and_never_sends_the_affected_attempt(
        endpoint: Uri,
        source_failure: ExternalTokenError,
        #[case] previous_status: Option<u16>,
    ) {
        let mut plan = match previous_status {
            Some(status) => Plan::new(&["A"], &[status]),
            None => Plan::new(&[], &[]),
        };
        plan.source.push(Err(source_failure.clone()));
        let outcome = plan.run(endpoint.clone(), endpoint).await;

        assert_that!(outcome.result)
            .is_err()
            .is_equal_to(ExternalAuthError::Source(source_failure));
        assert_that!(outcome.attempts.len()).is_equal_to(usize::from(previous_status.is_some()));
    }

    #[rstest]
    #[case::other_origin("https://download.example/api/graphql")]
    #[case::other_path("https://platform.example/oauth/token")]
    #[tokio::test]
    async fn unrelated_destinations_do_not_read_or_receive_the_external_token(
        endpoint: Uri,
        #[case] destination: Uri,
    ) {
        let outcome = Plan::new(&[], &[200])
            .run(endpoint, destination.clone())
            .await;

        assert_that!(outcome.result)
            .is_ok()
            .is_equal_to(StatusCode::OK);
        assert_that!(outcome.attempts).is_equal_to(vec![Attempt {
            method: Method::POST,
            uri: destination,
            authorization: Some("must-be-replaced".to_owned()),
            sensitive: false,
            api_key: Some("must-be-removed".to_owned()),
            body: Bytes::from_static(b"original request body"),
            retries: "rover=retries:0".to_owned(),
        }]);
    }

    #[rstest]
    #[tokio::test]
    async fn readiness_does_not_read_the_source(endpoint: Uri) {
        let source = MockTokenService::new();
        let mut http = MockHttpService::new();
        expect_poll_ready!(http);
        let mut service = ExternalAuthLayer::new(MockCloneService::new(source), endpoint)
            .layer(MockCloneService::new(http));

        let ready = service.ready().await.map(|_| ());

        assert_that!(ready).is_ok().is_equal_to(());
    }

    #[rstest]
    #[tokio::test(start_paused = true)]
    async fn concurrent_operations_have_independent_resends_and_attempt_counters(endpoint: Uri) {
        let reads = Arc::new(AtomicU32::new(0));
        let captured_reads = reads.clone();
        let mut source = MockTokenService::new();
        expect_poll_ready!(source, 8);
        source.expect_call().times(8).returning(move |_| {
            ready(Ok(ExternalAccessToken::parse(
                format!("token-{}", captured_reads.fetch_add(1, Ordering::Relaxed)).as_bytes(),
            )
            .unwrap()))
        });
        let attempts = Arc::new(Mutex::new(Vec::new()));
        let captured = attempts.clone();
        let mut counts = HashMap::<String, usize>::new();
        let mut http = MockHttpService::new();
        expect_poll_ready!(http, 8);
        http.expect_call().times(8).returning(move |request| {
            let operation = request.headers()["operation"].to_str().unwrap().to_owned();
            let count = counts.entry(operation.clone()).or_default();
            let status = if *count == 0 { 401 } else { 200 };
            *count += 1;
            captured.lock().unwrap().push((
                operation,
                RetryCount::of(&request).0,
                request.headers()[AUTHORIZATION]
                    .to_str()
                    .unwrap()
                    .to_owned(),
            ));
            ready(Ok(Response::builder()
                .status(status)
                .body(Full::default())
                .unwrap()))
        });
        let http = MockCloneService::new(http).map_future(|future| async move {
            let response = future.await;
            tokio::time::sleep(Duration::from_millis(1)).await;
            response
        });
        let service = ServiceBuilder::new()
            .layer(ExternalAuthOperationLayer)
            .layer(ExternalAuthLayer::new(
                MockCloneService::new(source),
                endpoint.clone(),
            ))
            .service(http);
        let mut results = stream::iter(0..4)
            .map(|operation| {
                service.clone().oneshot(
                    Request::builder()
                        .uri(endpoint.clone())
                        .header("operation", operation.to_string())
                        .body(Full::default())
                        .unwrap(),
                )
            })
            .buffer_unordered(4)
            .map(|result| {
                result
                    .map(|response| response.status())
                    .map_err(|error| error.to_string())
            });

        assert_that!(results.next().now_or_never()).is_none();
        let mut first_attempts = attempts
            .lock()
            .unwrap()
            .iter()
            .map(|(operation, attempt, _)| (operation.clone(), *attempt))
            .collect::<Vec<_>>();
        first_attempts.sort();
        assert_that!(first_attempts).is_equal_to(
            (0..4)
                .map(|operation| (operation.to_string(), 0))
                .collect::<Vec<_>>(),
        );

        let results = results.collect::<Vec<_>>().await;
        let mut actual = attempts
            .lock()
            .unwrap()
            .iter()
            .map(|(operation, attempt, _)| (operation.clone(), *attempt))
            .collect::<Vec<_>>();
        actual.sort();
        let expected = (0..4)
            .flat_map(|operation| [(operation.to_string(), 0), (operation.to_string(), 1)])
            .collect::<Vec<_>>();
        let unique_tokens = attempts
            .lock()
            .unwrap()
            .iter()
            .map(|(_, _, token)| token.clone())
            .collect::<HashSet<_>>();

        assert_that!(results).is_equal_to(vec![Ok(StatusCode::OK); 4]);
        assert_that!(actual).is_equal_to(expected);
        assert_that!(reads.load(Ordering::Relaxed)).is_equal_to(8);
        assert_that!(unique_tokens.len()).is_equal_to(8);
    }

    #[rstest]
    #[tokio::test(start_paused = true)]
    async fn source_reads_are_outside_the_timeout_and_each_send_has_its_own_timeout(endpoint: Uri) {
        let mut tokens = VecDeque::from([
            ExternalAccessToken::parse(b"A").unwrap(),
            ExternalAccessToken::parse(b"B").unwrap(),
        ]);
        let mut source = MockTokenService::new();
        expect_poll_ready!(source, 2);
        source
            .expect_call()
            .times(2)
            .returning(move |_| ready(Ok(tokens.pop_front().unwrap())));
        let source = MockCloneService::new(source).map_future(|future| async move {
            tokio::time::sleep(Duration::from_secs(2)).await;
            future.await
        });
        let mut statuses = VecDeque::from([StatusCode::UNAUTHORIZED, StatusCode::OK]);
        let mut http = MockHttpService::new();
        expect_poll_ready!(http, 2);
        http.expect_call().times(2).returning(move |_| {
            ready(Ok(Response::builder()
                .status(statuses.pop_front().unwrap())
                .body(Full::default())
                .unwrap()))
        });
        let http = MockCloneService::new(http).map_future(|future| async move {
            tokio::time::sleep(Duration::from_millis(750)).await;
            future.await
        });
        let service = ServiceBuilder::new()
            .layer(ExternalAuthOperationLayer)
            .layer(ExternalAuthLayer::new(source, endpoint.clone()))
            .layer(TimeoutLayer::new(Duration::from_secs(1)))
            .service(http);

        let result = service
            .oneshot(
                Request::builder()
                    .uri(endpoint)
                    .body(Full::default())
                    .unwrap(),
            )
            .await;

        assert_that!(result.map(|response| response.status()))
            .is_ok()
            .is_equal_to(StatusCode::OK);
    }

    #[rstest]
    #[case::source(ExternalAuthError::Source(ExternalTokenError {
        origin: ExternalTokenOrigin::Environment,
        path: "application/access-token".into(),
        kind: ExternalTokenFailure::Missing,
    }))]
    #[case::rejection(ExternalAuthError::Rejected)]
    #[case::permission_denied(ExternalAuthError::PermissionDenied)]
    fn external_auth_errors_round_trip_through_http_errors(#[case] diagnostic: ExternalAuthError) {
        let error = HttpServiceError::from(diagnostic.clone());

        assert_that!(matches!(&error, HttpServiceError::Unexpected(_))).is_equal_to(true);
        assert_that!(<&ExternalAuthError>::try_from(&error))
            .is_ok()
            .is_equal_to(&diagnostic);
    }

    #[rstest]
    #[case::transport_timeout(HttpServiceError::TimedOut)]
    #[case::other_boxed_error(HttpServiceError::Unexpected(Box::new(std::io::Error::other(
        "unrelated"
    ))))]
    #[case::other_wrapper(HttpServiceError::Decode(Box::new(ExternalAuthError::Rejected)))]
    fn unrelated_http_errors_are_not_external_authentication_failures(
        #[case] error: HttpServiceError,
    ) {
        assert_that!(<&ExternalAuthError>::try_from(&error)
            .map_err(|original| std::ptr::eq(original, &error)))
        .is_err()
        .is_equal_to(true);
    }
}

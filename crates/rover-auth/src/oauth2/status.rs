use bytes::Bytes;
use http::Method;
use oauth2::AccessToken;
use rover_http::{Full, HttpRequest, HttpResponse, body::body_to_bytes};
use rover_tower::{ResponseFuture, service::replace_ready_service};
use serde::Deserialize;
use tower::{Service, ServiceExt};
use url::Url;

/// Request to fetch the authenticated user's identity.
#[derive(Clone, Debug)]
pub struct WhoamiRequest {
    whoami_url: Url,
    access_token: AccessToken,
}

impl WhoamiRequest {
    /// Creates a new [`WhoamiRequest`].
    pub const fn new(whoami_url: Url, access_token: AccessToken) -> WhoamiRequest {
        WhoamiRequest {
            whoami_url,
            access_token,
        }
    }
}

/// Errors from the [`Whoami`] service.
#[derive(thiserror::Error, Debug)]
pub enum WhoamiError {
    /// HTTP transport error.
    #[error(transparent)]
    Http(Box<dyn std::error::Error + Send>),
    /// Failed to deserialize the server response.
    #[error("Failed to deserialize response: {}.", .0)]
    Deserialize(serde_json::Error),
    /// The access token was rejected (401).
    #[error("User is not logged in")]
    NotLoggedIn,
}

/// Authenticated user identity returned by the whoami endpoint.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct WhoamiResponse {
    /// The authenticated user's ID.
    pub user_id: String,
    /// The authenticated user's email.
    pub email: String,
    /// The authenticated user's display name.
    pub name: String,
}

/// Tower service that fetches the authenticated user's identity.
pub struct Whoami<S> {
    inner: S,
}

impl<S> Whoami<S> {
    /// Creates a new [`Whoami`] wrapping the given HTTP service.
    pub const fn new(inner: S) -> Whoami<S> {
        Whoami { inner }
    }
}

impl<S> Whoami<S>
where
    S: Service<HttpRequest, Response = HttpResponse> + Clone + Send + 'static,
    S::Error: std::error::Error + Send + 'static,
    S::Future: Send,
{
    /// Fetches the authenticated user's identity using the given service and request.
    pub async fn fetch(service: S, req: WhoamiRequest) -> Result<WhoamiResponse, WhoamiError> {
        Whoami::new(service).oneshot(req).await
    }
}

impl<S> Service<WhoamiRequest> for Whoami<S>
where
    S: Service<HttpRequest, Response = HttpResponse> + Clone + Send + 'static,
    S::Error: std::error::Error + Send + 'static,
    S::Future: Send,
{
    type Response = WhoamiResponse;
    type Error = WhoamiError;
    type Future = ResponseFuture<Result<Self::Response, Self::Error>>;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner
            .poll_ready(cx)
            .map_err(|err| WhoamiError::Http(Box::new(err)))
    }

    fn call(&mut self, req: WhoamiRequest) -> Self::Future {
        let mut inner = replace_ready_service(&mut self.inner);
        let fut = async move {
            let request = http::Request::builder()
                .uri(req.whoami_url.as_str())
                .method(Method::GET)
                .header(
                    http::header::AUTHORIZATION,
                    format!("Bearer {}", req.access_token.secret()),
                )
                .header(
                    http::header::ACCEPT,
                    http::HeaderValue::from_static("application/json"),
                )
                .body(Full::new(Bytes::new()))
                .map_err(|err| WhoamiError::Http(Box::new(err)))?;

            let mut resp = inner
                .call(request)
                .await
                .map_err(|err| WhoamiError::Http(Box::new(err)))?;

            match resp.status() {
                http::StatusCode::UNAUTHORIZED => Err(WhoamiError::NotLoggedIn),
                s if !s.is_success() => Err(WhoamiError::Http(Box::new(std::io::Error::other(
                    format!("unexpected HTTP status: {s}"),
                )))),
                _ => {
                    let body = body_to_bytes(resp.body_mut())
                        .await
                        .map_err(|err| WhoamiError::Http(Box::new(err)))?;
                    let resp = serde_json::from_slice(&body).map_err(WhoamiError::Deserialize)?;
                    Ok(resp)
                }
            }
        };
        Box::pin(fut)
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use bytes::Bytes;
    use http::{Method, Uri};
    use oauth2::AccessToken;
    use rover_http::{Full, HttpServiceError, test::MockHttpService};
    use rover_tower::{expect_poll_ready, test::MockCloneService};
    use rstest::{fixture, rstest};
    use speculoos::prelude::*;
    use tower::ServiceExt;
    use url::Url;

    use super::{Whoami, WhoamiError, WhoamiRequest};

    const WHOAMI_URL: &str = "https://test.com/api/auth/whoami";

    fn whoami_url() -> Url {
        Url::parse(WHOAMI_URL).unwrap()
    }

    #[fixture]
    fn access_token() -> AccessToken {
        AccessToken::new("test_bearer_token".to_string())
    }

    #[fixture]
    fn http_service() -> MockHttpService {
        MockHttpService::new()
    }

    fn whoami_200(user_id: &str, email: &str, name: &str) -> http::Response<Full<Bytes>> {
        let body = serde_json::json!({
            "user_id": user_id,
            "email": email,
            "name": name,
        })
        .to_string();
        http::Response::builder()
            .status(200)
            .body(Full::new(Bytes::from(body)))
            .unwrap()
    }

    #[rstest]
    #[tokio::test]
    #[timeout(Duration::from_secs(5))]
    async fn test_whoami_success(access_token: AccessToken, mut http_service: MockHttpService) {
        expect_poll_ready!(http_service);

        http_service
            .expect_call()
            .times(1)
            .withf(|req| {
                req.method() == Method::GET
                    && req.uri() == &Uri::try_from(WHOAMI_URL).unwrap()
                    && req
                        .headers()
                        .get(http::header::AUTHORIZATION)
                        .and_then(|v| v.to_str().ok())
                        .is_some_and(|v| v.starts_with("Bearer "))
            })
            .returning(|_| {
                futures::future::ready(Ok(whoami_200("user-123", "test@example.com", "Test User")))
            });

        let req = WhoamiRequest::new(whoami_url(), access_token);
        let result = Whoami::new(MockCloneService::new(http_service))
            .oneshot(req)
            .await;

        assert_that!(result).is_ok().matches(|resp| {
            resp.user_id == "user-123"
                && resp.email == "test@example.com"
                && resp.name == "Test User"
        });
    }

    #[rstest]
    #[tokio::test]
    #[timeout(Duration::from_secs(5))]
    async fn test_bearer_token_propagated(mut http_service: MockHttpService) {
        expect_poll_ready!(http_service);

        let secret = "my_secret";
        http_service
            .expect_call()
            .times(1)
            .withf(move |req| {
                req.headers()
                    .get(http::header::AUTHORIZATION)
                    .and_then(|v| v.to_str().ok())
                    .is_some_and(|v| v == format!("Bearer {secret}"))
            })
            .returning(|_| {
                futures::future::ready(Ok(whoami_200("user-123", "test@example.com", "Test User")))
            });

        let req = WhoamiRequest::new(whoami_url(), AccessToken::new(secret.to_string()));
        let result = Whoami::new(MockCloneService::new(http_service))
            .oneshot(req)
            .await;
        assert_that!(result).is_ok();
    }

    #[rstest]
    #[tokio::test]
    #[timeout(Duration::from_secs(5))]
    async fn test_whoami_http_error(access_token: AccessToken, mut http_service: MockHttpService) {
        expect_poll_ready!(http_service);

        http_service
            .expect_call()
            .times(1)
            .returning(|_| futures::future::ready(Err(HttpServiceError::TimedOut)));

        let req = WhoamiRequest::new(whoami_url(), access_token);
        let result = Whoami::new(MockCloneService::new(http_service))
            .oneshot(req)
            .await;

        assert_that!(result)
            .is_err()
            .matches(|e| matches!(e, WhoamiError::Http(_)));
    }

    #[rstest]
    #[tokio::test]
    #[timeout(Duration::from_secs(5))]
    async fn test_whoami_unauthorized(
        access_token: AccessToken,
        mut http_service: MockHttpService,
    ) {
        expect_poll_ready!(http_service);

        http_service.expect_call().times(1).returning(|_| {
            futures::future::ready(Ok(http::Response::builder()
                .status(401)
                .body(Full::new(Bytes::new()))
                .unwrap()))
        });

        let req = WhoamiRequest::new(whoami_url(), access_token);
        let result = Whoami::new(MockCloneService::new(http_service))
            .oneshot(req)
            .await;

        assert_that!(result)
            .is_err()
            .matches(|e| matches!(e, WhoamiError::NotLoggedIn));
    }

    #[rstest]
    #[tokio::test]
    #[timeout(Duration::from_secs(5))]
    async fn test_whoami_invalid_response(
        access_token: AccessToken,
        mut http_service: MockHttpService,
    ) {
        expect_poll_ready!(http_service);

        http_service.expect_call().times(1).returning(|_| {
            futures::future::ready(Ok(http::Response::builder()
                .status(200)
                .body(Full::new(Bytes::from("not valid json")))
                .unwrap()))
        });

        let req = WhoamiRequest::new(whoami_url(), access_token);
        let result = Whoami::new(MockCloneService::new(http_service))
            .oneshot(req)
            .await;

        assert_that!(result)
            .is_err()
            .matches(|e| matches!(e, WhoamiError::Deserialize(_)));
    }

    #[rstest]
    #[tokio::test]
    #[timeout(Duration::from_secs(5))]
    async fn test_whoami_non_2xx_error(
        access_token: AccessToken,
        mut http_service: MockHttpService,
    ) {
        expect_poll_ready!(http_service);

        http_service.expect_call().times(1).returning(|_| {
            futures::future::ready(Ok(http::Response::builder()
                .status(500)
                .body(Full::new(Bytes::new()))
                .unwrap()))
        });

        let req = WhoamiRequest::new(whoami_url(), access_token);
        let result = Whoami::new(MockCloneService::new(http_service))
            .oneshot(req)
            .await;

        assert_that!(result)
            .is_err()
            .matches(|e| matches!(e, WhoamiError::Http(_)));
    }
}

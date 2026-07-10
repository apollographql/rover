use std::{marker::PhantomData, time::Duration};

use http::{Request, Response};
use oauth2::{
    AccessToken, ClientId, RefreshToken as OauthRefreshToken, RequestTokenError, TokenResponse,
    TokenUrl, basic::BasicClient,
};
use rover_http::Body;
use rover_tower::{ResponseFuture, service::replace_ready_service};
use tower::Service;
use url::Url;

use crate::OauthHttpClient;

/// Errors from the [`RefreshToken`] service.
#[derive(thiserror::Error, Debug)]
pub enum RefreshTokenError {
    /// HTTP transport error.
    #[error(transparent)]
    Http(Box<dyn std::error::Error + Send>),
}

/// Request to exchange a refresh token for a new access token.
pub struct RefreshTokenRequest {
    client_id: ClientId,
    token_url: TokenUrl,
    refresh_token: OauthRefreshToken,
}

#[bon::bon]
impl RefreshTokenRequest {
    /// Creates a new [`RefreshTokenRequest`].
    #[builder]
    pub fn new(client_id: String, token_url: Url, refresh_token: String) -> RefreshTokenRequest {
        RefreshTokenRequest {
            client_id: ClientId::new(client_id),
            token_url: TokenUrl::from_url(token_url),
            refresh_token: OauthRefreshToken::new(refresh_token),
        }
    }
}

/// Successful response from a token refresh.
#[derive(Debug)]
pub struct RefreshTokenResponse {
    /// The new access token.
    pub access_token: AccessToken,
    /// A replacement refresh token, if issued.
    pub refresh_token: Option<OauthRefreshToken>,
    /// Lifetime of the new access token.
    pub expires_in: Option<Duration>,
}

/// Tower service that exchanges a refresh token for a new access token.
pub struct RefreshToken<S, B> {
    inner: S,
    _body: PhantomData<B>,
}

impl<S, B> RefreshToken<S, B> {
    /// Creates a new [`RefreshToken`] wrapping the given HTTP service.
    pub const fn new(inner: S) -> RefreshToken<S, B> {
        RefreshToken {
            inner,
            _body: PhantomData,
        }
    }
}

impl<S, B> Service<RefreshTokenRequest> for RefreshToken<S, B>
where
    S: Service<Request<B>, Response = Response<B>> + Clone + Send + 'static,
    S::Error: std::error::Error + From<B::Error> + Send + 'static,
    S::Future: Send,
    B: From<Vec<u8>> + Body + Unpin + Send + Sync + 'static,
    B::Data: Send,
{
    type Response = RefreshTokenResponse;
    type Error = RefreshTokenError;
    type Future = ResponseFuture<Result<Self::Response, Self::Error>>;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner
            .poll_ready(cx)
            .map_err(|err| RefreshTokenError::Http(Box::new(err)))
    }

    fn call(&mut self, req: RefreshTokenRequest) -> Self::Future {
        let service = replace_ready_service(&mut self.inner);

        let fut = async move {
            let http_client = OauthHttpClient::new(service);
            let client = BasicClient::new(req.client_id).set_token_uri(req.token_url);
            let resp = client
                .exchange_refresh_token(&req.refresh_token)
                .request_async(&http_client)
                .await
                .map_err(|err| match err {
                    RequestTokenError::Request(e) => RefreshTokenError::Http(Box::new(e)),
                    other => RefreshTokenError::Http(Box::new(other)),
                })?;
            Ok(RefreshTokenResponse {
                access_token: resp.access_token().clone(),
                refresh_token: resp.refresh_token().cloned(),
                expires_in: resp.expires_in(),
            })
        };
        Box::pin(fut)
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use bytes::Bytes;
    use http::{Method, Uri};
    use rover_http::{Full, HttpServiceError, test::MockHttpService};
    use rover_tower::{expect_poll_ready, test::MockCloneService};
    use rstest::{fixture, rstest};
    use speculoos::prelude::*;
    use tower::{Service, ServiceExt};
    use url::Url;

    use crate::oauth2::refresh_token::{RefreshToken, RefreshTokenError, RefreshTokenRequest};

    #[fixture]
    fn client_id() -> String {
        "client_id".to_string()
    }

    #[fixture]
    fn token_url() -> Url {
        Url::parse("https://example.com/token").unwrap()
    }

    #[fixture]
    fn http_service() -> MockHttpService {
        MockHttpService::new()
    }

    fn token_200() -> http::Response<Full<Bytes>> {
        let body = serde_json::json!({
            "access_token": "new_access_token",
            "refresh_token": "new_refresh_token",
            "token_type": "Bearer"
        });
        http::Response::builder()
            .body(Full::new(Bytes::from(serde_json::to_vec(&body).unwrap())))
            .unwrap()
    }

    #[rstest]
    #[tokio::test]
    #[timeout(Duration::from_secs(5))]
    async fn test_refresh_token_success(
        client_id: String,
        token_url: Url,
        mut http_service: MockHttpService,
    ) {
        expect_poll_ready!(http_service, 1);

        // GIVEN: HTTP request succeeds to refresh the token
        let expected_token_url = token_url.clone();
        http_service
            .expect_call()
            .times(1)
            .withf(move |req| {
                req.method() == Method::POST
                    && req.uri() == &Uri::try_from(expected_token_url.as_str()).unwrap()
            })
            .returning(|_| futures::future::ready(Ok(token_200())));

        let req = RefreshTokenRequest::builder()
            .client_id(client_id)
            .token_url(token_url)
            .refresh_token("old_refresh_token".to_string())
            .build();

        let mut service: RefreshToken<_, Full<Bytes>> =
            RefreshToken::new(MockCloneService::new(http_service));
        let service = service.ready().await.unwrap();
        let result = service.call(req).await;
        let resp = assert_that!(result).is_ok().subject;

        // THEN: The new token is returned
        assert_that!(resp.access_token.secret()).is_equal_to(&"new_access_token".to_string());
        assert_that!(resp.refresh_token.as_ref().unwrap().secret())
            .is_equal_to(&"new_refresh_token".to_string());
    }

    #[rstest]
    #[tokio::test]
    #[timeout(Duration::from_secs(5))]
    async fn test_refresh_token_http_error(
        client_id: String,
        token_url: Url,
        mut http_service: MockHttpService,
    ) {
        expect_poll_ready!(http_service, 1);

        // GIVEN: HTTP request fails to refresh the token
        http_service
            .expect_call()
            .times(1)
            .returning(|_| futures::future::ready(Err(HttpServiceError::TimedOut)));

        let req = RefreshTokenRequest::builder()
            .client_id(client_id)
            .token_url(token_url)
            .refresh_token("old_refresh_token".to_string())
            .build();

        let mut service: RefreshToken<_, Full<Bytes>> =
            RefreshToken::new(MockCloneService::new(http_service));
        let service = service.ready().await.unwrap();
        let result = service.call(req).await;

        // THEN: The error is returned
        assert_that!(result)
            .is_err()
            .matches(|e| matches!(e, RefreshTokenError::Http(inner) if inner.to_string().contains("Request timed out")));
    }

    #[rstest]
    #[tokio::test]
    #[timeout(Duration::from_secs(5))]
    async fn test_poll_ready_error_propagates(mut http_service: MockHttpService) {
        // GIVEN: HTTP request fails to poll ready
        http_service
            .expect_poll_ready()
            .times(1)
            .returning(|_| std::task::Poll::Ready(Err(HttpServiceError::TimedOut)));

        let mut service: RefreshToken<_, Full<Bytes>> =
            RefreshToken::new(MockCloneService::new(http_service));
        let result = service.ready().await;

        // THEN: The error is returned
        assert_that!(result.err())
            .is_some()
            .matches(|e| matches!(e, RefreshTokenError::Http(inner) if inner.to_string().contains("Request timed out")));
    }
}

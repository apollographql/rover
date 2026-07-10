use std::marker::PhantomData;

use http::{Request, Response};
use oauth2::{
    ClientId, RequestTokenError, RevocationUrl, StandardRevocableToken, basic::BasicClient,
};
use rover_http::Body;
use rover_tower::{ResponseFuture, service::replace_ready_service};
use tower::Service;
use url::Url;

use crate::OauthHttpClient;

/// Errors from the [`RevokeToken`] service.
#[derive(thiserror::Error, Debug)]
pub enum RevokeTokenError {
    /// HTTP transport error.
    #[error(transparent)]
    Http(Box<dyn std::error::Error + Send>),
    /// Invalid oauth2 client configuration for revocation.
    #[error("Failed to configure revoke token request. {}", .0)]
    OauthConfiguration(#[from] oauth2::ConfigurationError),
}

/// Request to revoke an access or refresh token.
pub struct RevokeTokenRequest {
    client_id: ClientId,
    revocation_url: RevocationUrl,
    token: StandardRevocableToken,
}

#[bon::bon]
impl RevokeTokenRequest {
    /// Creates a new [`RevokeTokenRequest`].
    #[builder]
    pub fn new<RC>(client_id: String, revocation_url: Url, token: RC) -> RevokeTokenRequest
    where
        RC: Into<StandardRevocableToken>,
    {
        RevokeTokenRequest {
            client_id: ClientId::new(client_id),
            revocation_url: RevocationUrl::from_url(revocation_url),
            token: token.into(),
        }
    }
}

/// Successful response from token revocation (empty body per RFC 7009).
#[derive(Debug)]
pub struct RevokeTokenResponse {}

/// Tower service that revokes an OAuth2 token (RFC 7009).
pub struct RevokeToken<S, B> {
    inner: S,
    _body: PhantomData<B>,
}

impl<S, B> std::fmt::Debug for RevokeToken<S, B>
where
    S: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RevokeToken")
            .field("inner", &self.inner)
            .finish()
    }
}

impl<S, B> RevokeToken<S, B> {
    /// Creates a new [`RevokeToken`] wrapping the given HTTP service.
    pub const fn new(inner: S) -> RevokeToken<S, B> {
        RevokeToken {
            inner,
            _body: PhantomData,
        }
    }
}

impl<S, B> Service<RevokeTokenRequest> for RevokeToken<S, B>
where
    S: Service<Request<B>, Response = Response<B>> + Clone + Send + 'static,
    S::Error: std::error::Error + From<B::Error> + Send + 'static,
    S::Future: Send,
    B: From<Vec<u8>> + Body + Unpin + Send + Sync + 'static,
    B::Data: Send,
{
    type Response = RevokeTokenResponse;
    type Error = RevokeTokenError;
    type Future = ResponseFuture<Result<Self::Response, Self::Error>>;
    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner
            .poll_ready(cx)
            .map_err(|err| RevokeTokenError::Http(Box::new(err)))
    }

    fn call(&mut self, req: RevokeTokenRequest) -> Self::Future {
        let service = replace_ready_service(&mut self.inner);

        let fut = async move {
            let http_client = OauthHttpClient::new(service);
            let oauth_client = BasicClient::new(req.client_id.clone())
                .set_revocation_url(req.revocation_url.clone());
            let request = oauth_client.revoke_token(req.token)?;
            request
                .request_async(&http_client)
                .await
                .map_err(|err| match err {
                    RequestTokenError::Request(e) => RevokeTokenError::Http(Box::new(e)),
                    other => RevokeTokenError::Http(Box::new(other)),
                })?;
            Ok(RevokeTokenResponse {})
        };
        Box::pin(fut)
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use bytes::Bytes;
    use http::{Method, Uri};
    use oauth2::{AccessToken, RefreshToken};
    use rover_http::{Full, HttpServiceError, test::MockHttpService};
    use rover_tower::{expect_poll_ready, test::MockCloneService};
    use rstest::{fixture, rstest};
    use speculoos::prelude::*;
    use tower::{Service, ServiceExt};
    use url::Url;

    use crate::oauth2::revoke_token::{RevokeToken, RevokeTokenError, RevokeTokenRequest};

    #[fixture]
    fn client_id() -> String {
        "client_id".to_string()
    }

    #[fixture]
    fn revocation_url() -> Url {
        Url::parse("https://example.com/revoke").unwrap()
    }

    #[fixture]
    fn http_service() -> MockHttpService {
        MockHttpService::new()
    }

    fn empty_200() -> http::Response<Full<Bytes>> {
        http::Response::builder()
            .body(Full::new(Bytes::new()))
            .unwrap()
    }

    #[rstest]
    #[tokio::test]
    #[timeout(Duration::from_secs(5))]
    async fn test_revoke_access_token_success(
        client_id: String,
        revocation_url: Url,
        mut http_service: MockHttpService,
    ) {
        expect_poll_ready!(http_service, 1);

        let expected_revocation_url = revocation_url.clone();
        http_service
            .expect_call()
            .times(1)
            .withf(move |req| {
                req.method() == Method::POST
                    && req.uri() == &Uri::try_from(expected_revocation_url.as_str()).unwrap()
            })
            .returning(|_| futures::future::ready(Ok(empty_200())));

        let token = AccessToken::new("access_token".to_string());
        let req = RevokeTokenRequest::builder()
            .client_id(client_id)
            .revocation_url(revocation_url)
            .token(token)
            .build();

        let mut service: RevokeToken<_, Full<Bytes>> =
            RevokeToken::new(MockCloneService::new(http_service));
        let service = service.ready().await.unwrap();
        let result = service.call(req).await;
        assert_that!(result).is_ok();
    }

    #[rstest]
    #[tokio::test]
    #[timeout(Duration::from_secs(5))]
    async fn test_revoke_refresh_token_success(
        client_id: String,
        revocation_url: Url,
        mut http_service: MockHttpService,
    ) {
        expect_poll_ready!(http_service, 1);

        let expected_revocation_url = revocation_url.clone();
        http_service
            .expect_call()
            .times(1)
            .withf(move |req| {
                req.method() == Method::POST
                    && req.uri() == &Uri::try_from(expected_revocation_url.as_str()).unwrap()
            })
            .returning(|_| futures::future::ready(Ok(empty_200())));

        let token = RefreshToken::new("refresh_token".to_string());
        let req = RevokeTokenRequest::builder()
            .client_id(client_id)
            .revocation_url(revocation_url)
            .token(token)
            .build();

        let mut service: RevokeToken<_, Full<Bytes>> =
            RevokeToken::new(MockCloneService::new(http_service));
        let service = service.ready().await.unwrap();
        let result = service.call(req).await;
        assert_that!(result).is_ok();
    }

    #[rstest]
    #[tokio::test]
    #[timeout(Duration::from_secs(5))]
    async fn test_revoke_token_http_error(
        client_id: String,
        revocation_url: Url,
        mut http_service: MockHttpService,
    ) {
        expect_poll_ready!(http_service, 1);

        http_service
            .expect_call()
            .times(1)
            .returning(|_| futures::future::ready(Err(HttpServiceError::TimedOut)));

        let token = AccessToken::new("access_token".to_string());
        let req = RevokeTokenRequest::builder()
            .client_id(client_id)
            .revocation_url(revocation_url)
            .token(token)
            .build();

        let mut service: RevokeToken<_, Full<Bytes>> =
            RevokeToken::new(MockCloneService::new(http_service));
        let service = service.ready().await.unwrap();
        let result = service.call(req).await;
        assert_that!(result)
            .is_err()
            .matches(|e| matches!(e, RevokeTokenError::Http(inner) if inner.to_string().contains("Request timed out")));
    }

    #[rstest]
    #[tokio::test]
    #[timeout(Duration::from_secs(5))]
    async fn test_poll_ready_error_propagates(mut http_service: MockHttpService) {
        http_service
            .expect_poll_ready()
            .times(1)
            .returning(|_| std::task::Poll::Ready(Err(HttpServiceError::TimedOut)));

        let mut service: RevokeToken<_, Full<Bytes>> =
            RevokeToken::new(MockCloneService::new(http_service));
        let result = service.ready().await;
        assert_that!(result.err())
            .is_some()
            .matches(|e| matches!(e, RevokeTokenError::Http(inner) if inner.to_string().contains("Request timed out")));
    }
}

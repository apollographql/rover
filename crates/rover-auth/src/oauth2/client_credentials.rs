use std::{marker::PhantomData, time::Duration};

use http::{Request, Response};
use oauth2::{
    AccessToken, ClientId, ClientSecret, RequestTokenError, Scope, TokenResponse, TokenUrl,
    basic::{BasicClient, BasicErrorResponse},
};
use rover_http::Body;
use rover_tower::{ResponseFuture, service::replace_ready_service};
use tower::Service;
use url::Url;

use crate::OauthHttpClient;

/// Errors from the [`ClientCredentials`] service.
#[derive(thiserror::Error, Debug)]
pub enum ClientCredentialsError {
    /// HTTP transport error.
    #[error(transparent)]
    Http(Box<dyn std::error::Error + Send>),
    /// The token endpoint returned an OAuth 2.0 error (e.g. `invalid_client`, `invalid_scope`).
    #[error("{0}")]
    OAuth(BasicErrorResponse),
    /// The token endpoint response could not be parsed.
    #[error("failed to parse token endpoint response: {source}")]
    Parse {
        /// The underlying parse error.
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
        /// The raw response body that failed to parse.
        body: Vec<u8>,
    },
    /// An unexpected response from the token endpoint.
    #[error("{0}")]
    Other(String),
    /// `client_id` or `client_secret` must not be empty.
    #[error("{field} must not be empty")]
    EmptyCredential {
        /// The name of the field that was empty (`"client_id"` or `"client_secret"`).
        field: &'static str,
    },
}

/// Request to obtain an access token via the client credentials grant.
pub struct ClientCredentialsRequest {
    client_id: ClientId,
    client_secret: ClientSecret,
    token_url: TokenUrl,
    scopes: Vec<Scope>,
}

#[bon::bon]
impl ClientCredentialsRequest {
    /// Creates a new [`ClientCredentialsRequest`].
    #[builder]
    pub fn new(
        client_id: String,
        client_secret: String,
        token_url: Url,
        scopes: Vec<Scope>,
    ) -> Result<ClientCredentialsRequest, ClientCredentialsError> {
        if client_id.is_empty() {
            return Err(ClientCredentialsError::EmptyCredential { field: "client_id" });
        }
        if client_secret.is_empty() {
            return Err(ClientCredentialsError::EmptyCredential {
                field: "client_secret",
            });
        }
        Ok(ClientCredentialsRequest {
            client_id: ClientId::new(client_id),
            client_secret: ClientSecret::new(client_secret),
            token_url: TokenUrl::from_url(token_url),
            scopes,
        })
    }
}

/// Successful response from the client credentials token endpoint.
#[derive(Debug)]
pub struct ClientCredentialsResponse {
    /// The issued access token.
    pub access_token: AccessToken,
    /// Lifetime of the access token.
    pub expires_in: Option<Duration>,
}

/// Tower service that obtains an access token via the OAuth2 client credentials grant (RFC 6749 §4.4).
pub struct ClientCredentials<S, B> {
    inner: S,
    _body: PhantomData<B>,
}

impl<S, B> ClientCredentials<S, B> {
    /// Creates a new [`ClientCredentials`] wrapping the given HTTP service.
    pub const fn new(inner: S) -> ClientCredentials<S, B> {
        ClientCredentials {
            inner,
            _body: PhantomData,
        }
    }
}

impl<S, B> Service<ClientCredentialsRequest> for ClientCredentials<S, B>
where
    S: Service<Request<B>, Response = Response<B>> + Clone + Send + 'static,
    S::Error: std::error::Error + From<B::Error> + Send + 'static,
    S::Future: Send,
    B: From<Vec<u8>> + Body + Unpin + Send + Sync + 'static,
    B::Data: Send,
{
    type Response = ClientCredentialsResponse;
    type Error = ClientCredentialsError;
    type Future = ResponseFuture<Result<Self::Response, Self::Error>>;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner
            .poll_ready(cx)
            .map_err(|err| ClientCredentialsError::Http(Box::new(err)))
    }

    fn call(&mut self, req: ClientCredentialsRequest) -> Self::Future {
        let service = replace_ready_service(&mut self.inner);
        let fut = async move {
            let http_client = OauthHttpClient::new(service);
            let client = BasicClient::new(req.client_id)
                .set_client_secret(req.client_secret)
                .set_token_uri(req.token_url);
            let resp = client
                .exchange_client_credentials()
                .add_scopes(req.scopes)
                .request_async(&http_client)
                .await
                .map_err(|err| match err {
                    RequestTokenError::Request(e) => ClientCredentialsError::Http(Box::new(e)),
                    RequestTokenError::ServerResponse(resp) => ClientCredentialsError::OAuth(resp),
                    RequestTokenError::Parse(err, body) => ClientCredentialsError::Parse {
                        source: Box::new(err),
                        body,
                    },
                    RequestTokenError::Other(msg) => ClientCredentialsError::Other(msg),
                })?;
            Ok(ClientCredentialsResponse {
                access_token: resp.access_token().clone(),
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
    use oauth2::Scope;
    use rover_http::{Full, HttpServiceError, test::MockHttpService};
    use rover_tower::{expect_poll_ready, test::MockCloneService};
    use rstest::{fixture, rstest};
    use speculoos::prelude::*;
    use tower::{Service, ServiceExt};
    use url::Url;

    use crate::oauth2::client_credentials::{
        ClientCredentials, ClientCredentialsError, ClientCredentialsRequest,
    };

    #[fixture]
    fn client_id() -> String {
        "client_id".to_string()
    }

    #[fixture]
    fn client_secret() -> String {
        "client_secret".to_string()
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
            "access_token": "my_access_token",
            "token_type": "Bearer"
        });
        http::Response::builder()
            .body(Full::new(Bytes::from(serde_json::to_vec(&body).unwrap())))
            .unwrap()
    }

    fn token_400_invalid_client() -> http::Response<Full<Bytes>> {
        let body = serde_json::json!({
            "error": "invalid_client",
            "error_description": "Client authentication failed"
        });
        http::Response::builder()
            .status(400)
            .body(Full::new(Bytes::from(serde_json::to_vec(&body).unwrap())))
            .unwrap()
    }

    fn token_200_with_expires_in() -> http::Response<Full<Bytes>> {
        let body = serde_json::json!({
            "access_token": "my_access_token",
            "token_type": "Bearer",
            "expires_in": 3600
        });
        http::Response::builder()
            .body(Full::new(Bytes::from(serde_json::to_vec(&body).unwrap())))
            .unwrap()
    }

    #[rstest]
    #[tokio::test]
    #[timeout(Duration::from_secs(5))]
    async fn test_client_credentials_success(
        client_id: String,
        client_secret: String,
        token_url: Url,
        mut http_service: MockHttpService,
    ) {
        expect_poll_ready!(http_service, 1);

        let expected_token_url = token_url.clone();
        http_service
            .expect_call()
            .times(1)
            .withf(move |req| {
                req.method() == Method::POST
                    && req.uri() == &Uri::try_from(expected_token_url.as_str()).unwrap()
            })
            .returning(|_| futures::future::ready(Ok(token_200())));

        let req = ClientCredentialsRequest::builder()
            .client_id(client_id)
            .client_secret(client_secret)
            .token_url(token_url)
            .scopes(vec![Scope::new("rover:cli".to_string())])
            .build()
            .unwrap();

        let mut service: ClientCredentials<_, Full<Bytes>> =
            ClientCredentials::new(MockCloneService::new(http_service));
        let service = service.ready().await.unwrap();
        let result = service.call(req).await;
        let resp = assert_that!(result).is_ok().subject;

        assert_that!(resp.access_token.secret()).is_equal_to(&"my_access_token".to_string());
    }

    #[rstest]
    #[tokio::test]
    #[timeout(Duration::from_secs(5))]
    async fn test_client_credentials_success_populates_expires_in(
        client_id: String,
        client_secret: String,
        token_url: Url,
        mut http_service: MockHttpService,
    ) {
        expect_poll_ready!(http_service, 1);

        http_service
            .expect_call()
            .times(1)
            .returning(|_| futures::future::ready(Ok(token_200_with_expires_in())));

        let req = ClientCredentialsRequest::builder()
            .client_id(client_id)
            .client_secret(client_secret)
            .token_url(token_url)
            .scopes(vec![])
            .build()
            .unwrap();

        let mut service: ClientCredentials<_, Full<Bytes>> =
            ClientCredentials::new(MockCloneService::new(http_service));
        let service = service.ready().await.unwrap();
        let result = service.call(req).await;
        let resp = assert_that!(result).is_ok().subject;

        assert_that!(resp.expires_in).is_equal_to(Some(Duration::from_secs(3600)));
    }

    #[rstest]
    #[tokio::test]
    #[timeout(Duration::from_secs(5))]
    async fn test_client_credentials_oauth_error(
        client_id: String,
        client_secret: String,
        token_url: Url,
        mut http_service: MockHttpService,
    ) {
        expect_poll_ready!(http_service, 1);

        http_service
            .expect_call()
            .times(1)
            .returning(|_| futures::future::ready(Ok(token_400_invalid_client())));

        let req = ClientCredentialsRequest::builder()
            .client_id(client_id)
            .client_secret(client_secret)
            .token_url(token_url)
            .scopes(vec![])
            .build()
            .unwrap();

        let mut service: ClientCredentials<_, Full<Bytes>> =
            ClientCredentials::new(MockCloneService::new(http_service));
        let service = service.ready().await.unwrap();
        let result = service.call(req).await;

        assert_that!(result)
            .is_err()
            .matches(|e| matches!(e, ClientCredentialsError::OAuth(_)));
    }

    #[rstest]
    #[tokio::test]
    #[timeout(Duration::from_secs(5))]
    async fn test_client_credentials_http_error(
        client_id: String,
        client_secret: String,
        token_url: Url,
        mut http_service: MockHttpService,
    ) {
        expect_poll_ready!(http_service, 1);

        http_service
            .expect_call()
            .times(1)
            .returning(|_| futures::future::ready(Err(HttpServiceError::TimedOut)));

        let req = ClientCredentialsRequest::builder()
            .client_id(client_id)
            .client_secret(client_secret)
            .token_url(token_url)
            .scopes(vec![])
            .build()
            .unwrap();

        let mut service: ClientCredentials<_, Full<Bytes>> =
            ClientCredentials::new(MockCloneService::new(http_service));
        let service = service.ready().await.unwrap();
        let result = service.call(req).await;

        assert_that!(result)
            .is_err()
            .matches(|e| matches!(e, ClientCredentialsError::Http(inner) if inner.to_string().contains("Request timed out")));
    }

    #[rstest]
    #[tokio::test]
    #[timeout(Duration::from_secs(5))]
    async fn test_poll_ready_error_propagates(mut http_service: MockHttpService) {
        http_service
            .expect_poll_ready()
            .times(1)
            .returning(|_| std::task::Poll::Ready(Err(HttpServiceError::TimedOut)));

        let mut service: ClientCredentials<_, Full<Bytes>> =
            ClientCredentials::new(MockCloneService::new(http_service));
        let result = service.ready().await;

        assert_that!(result.err())
            .is_some()
            .matches(|e| matches!(e, ClientCredentialsError::Http(inner) if inner.to_string().contains("Request timed out")));
    }

    #[rstest]
    fn test_empty_client_id_is_rejected(client_secret: String, token_url: Url) {
        let err = ClientCredentialsRequest::builder()
            .client_id("".to_string())
            .client_secret(client_secret)
            .token_url(token_url)
            .scopes(vec![])
            .build()
            .err()
            .expect("expected an error");

        assert!(matches!(
            err,
            ClientCredentialsError::EmptyCredential { field: "client_id" }
        ));
    }

    #[rstest]
    fn test_empty_client_secret_is_rejected(client_id: String, token_url: Url) {
        let err = ClientCredentialsRequest::builder()
            .client_id(client_id)
            .client_secret("".to_string())
            .token_url(token_url)
            .scopes(vec![])
            .build()
            .err()
            .expect("expected an error");

        assert!(matches!(
            err,
            ClientCredentialsError::EmptyCredential {
                field: "client_secret"
            }
        ));
    }
}

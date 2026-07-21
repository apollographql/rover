use std::fmt::Debug;

use oauth2::{
    AccessToken, AuthUrl, ClientId, CsrfToken, EndpointNotSet, EndpointSet, PkceCodeChallenge,
    RedirectUrl, RefreshToken, Scope, TokenResponse, TokenUrl, basic::BasicClient,
};
use redirect::server::{RedirectServerAwait, RedirectServerBind, RedirectServerError};
use rover_http::Body;
use rover_open::OpenUrl;
use rover_print::{
    print::Print,
    style::{Style, StyledText},
};
use tower::Service;
use url::Url;

use crate::OauthHttpClient;

/// OAuth redirect server for handling the PKCE callback.
pub mod redirect;

type AuthorizationFlowClient =
    BasicClient<EndpointSet, EndpointNotSet, EndpointNotSet, EndpointNotSet, EndpointSet>;

/// Tokens returned after a successful PKCE authorization code exchange.
#[derive(Debug)]
pub struct AuthorizationFlowResponse {
    /// The issued access token.
    pub access_token: AccessToken,
    /// A refresh token, if the server issued one.
    pub refresh_token: Option<RefreshToken>,
    /// Lifetime of the access token.
    pub expires_in: Option<std::time::Duration>,
}

impl PartialEq for AuthorizationFlowResponse {
    fn eq(&self, other: &Self) -> bool {
        self.access_token.secret() == other.access_token.secret()
            && self.expires_in == other.expires_in
            && self
                .refresh_token
                .as_ref()
                .map(|refresh_token| refresh_token.secret())
                == other
                    .refresh_token
                    .as_ref()
                    .map(|refresh_token| refresh_token.secret())
    }
}

mod state {
    use oauth2::{AuthorizationCode, PkceCodeVerifier};
    use url::Url;

    use super::AuthorizationFlowClient;

    #[derive(Debug)]
    pub struct AuthorizationFlowInit {
        pub client_id: String,
        pub auth_url: Url,
        pub token_url: Url,
    }

    #[derive(Debug)]
    pub struct AuthorizationFlowWithCode {
        pub code: AuthorizationCode,
        pub pkce_verifier: PkceCodeVerifier,
        pub client: AuthorizationFlowClient,
    }
}

/// Errors from the PKCE authorization flow.
#[derive(thiserror::Error, Debug)]
pub enum AuthorizationFlowError {
    /// The local redirect server failed.
    #[error(transparent)]
    RedirectServer(#[from] RedirectServerError),
    /// Could not construct the redirect URL from the server address.
    #[error("Failed to parse the redirect server URL: {}", .0)]
    RedirectUrl(url::ParseError),
    /// The token endpoint rejected the authorization code.
    #[error("Failed to exchange access code: {}", .0)]
    AccessCodeExchange(Box<dyn std::error::Error>),
}

/// State machine for the OAuth2 PKCE authorization code flow.
#[derive(Debug)]
pub struct AuthorizationFlow<T>
where
    T: Debug,
{
    state: T,
}

#[bon::bon]
impl AuthorizationFlow<state::AuthorizationFlowInit> {
    #[builder]
    /// Creates a new [`AuthorizationFlow`] in its initial state.
    pub const fn new(
        client_id: String,
        authorization_url: Url,
        token_url: Url,
    ) -> AuthorizationFlow<state::AuthorizationFlowInit> {
        AuthorizationFlow {
            state: state::AuthorizationFlowInit {
                client_id,
                auth_url: authorization_url,
                token_url,
            },
        }
    }
    /// Opens the authorization URL and waits for the OAuth callback, returning the flow with code.
    pub async fn authorize<O, P, RS>(
        &self,
        scopes: Vec<Scope>,
        open_auth_url: &O,
        stderr: &P,
        redirect_server: RS,
    ) -> Result<AuthorizationFlow<state::AuthorizationFlowWithCode>, AuthorizationFlowError>
    where
        O: OpenUrl,
        P: Print,
        RS: RedirectServerBind,
    {
        let redirect_server = redirect_server.bind().await?;
        let addr = redirect_server.local_addr()?;
        let redirect_url = RedirectUrl::new(format!("http://{}:{}/", addr.ip(), addr.port()))
            .map_err(AuthorizationFlowError::RedirectUrl)?;
        let client = BasicClient::new(ClientId::new(self.state.client_id.clone()))
            .set_redirect_uri(redirect_url)
            .set_token_uri(TokenUrl::from_url(self.state.token_url.clone()))
            .set_auth_uri(AuthUrl::from_url(self.state.auth_url.clone()));
        let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();
        let (auth_url, csrf_token) = client
            .authorize_url(CsrfToken::new_random)
            .add_scopes(scopes)
            .set_pkce_challenge(pkce_challenge)
            .add_extra_param("prompt", "login")
            .url();

        if let Err(err) = open_auth_url.open_url(&auth_url) {
            tracing::error!("Failed to open URL automatically: {}", err);
            if let Err(print_err) = stderr.print(&StyledText::new(Style::Error, format!("We were unable to open the OAuth Authorization URL automatically. Open {} in a browser to continue.", auth_url))) {
                tracing::error!("Failed to print message: {}", print_err);
            }
        }
        let code = redirect_server.await_response(csrf_token).await?;
        Ok(AuthorizationFlow {
            state: state::AuthorizationFlowWithCode {
                code,
                pkce_verifier,
                client,
            },
        })
    }
}

impl AuthorizationFlow<state::AuthorizationFlowWithCode> {
    /// Exchanges the authorization code for tokens via the token endpoint.
    pub async fn exchange_code<S, B>(
        self,
        http_service: S,
    ) -> Result<AuthorizationFlowResponse, AuthorizationFlowError>
    where
        S: Service<http::Request<B>, Response = http::Response<B>> + Send + 'static,
        S::Error: std::error::Error + From<B::Error> + 'static,
        S::Future: Send,
        B: From<Vec<u8>> + Body + Unpin + Send,
        B::Data: Send,
    {
        let http_client = OauthHttpClient::new(http_service);
        let resp = self
            .state
            .client
            .exchange_code(self.state.code)
            .set_pkce_verifier(self.state.pkce_verifier)
            .request_async(&http_client)
            .await
            .map_err(|err| AuthorizationFlowError::AccessCodeExchange(Box::new(err)))?;

        let access_token = resp.access_token().clone();
        let refresh_token = resp.refresh_token().cloned();
        let expires_in = resp.expires_in();
        Ok(AuthorizationFlowResponse {
            access_token,
            refresh_token,
            expires_in,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::{
        net::{IpAddr, Ipv4Addr, SocketAddr},
        time::Duration,
    };

    use bytes::Bytes;
    use http::{Method, Uri};
    use oauth2::{AccessToken, AuthorizationCode, RefreshToken, Scope};
    use rover_http::{Full, test::MockHttpService};
    use rover_open::MockOpenUrl;
    use rover_print::{print::MockPrint, style::Style};
    use rstest::{fixture, rstest};
    use speculoos::prelude::*;
    use url::Url;

    use crate::oauth2::authorization_flow::{
        AuthorizationFlow, AuthorizationFlowResponse,
        redirect::server::{MockRedirectServerAwait, MockRedirectServerBind},
    };

    #[fixture]
    fn auth_url() -> Url {
        Url::parse("https://example.com/authorize").unwrap()
    }

    #[fixture]
    fn token_url() -> Url {
        Url::parse("https://example.com/token").unwrap()
    }

    #[fixture]
    fn client_id() -> String {
        "client_id".to_string()
    }

    #[fixture]
    fn http_service() -> MockHttpService {
        MockHttpService::new()
    }

    #[rstest]
    #[tokio::test]
    #[timeout(Duration::from_secs(5))]
    async fn test_pkce_flow_success(
        client_id: String,
        auth_url: Url,
        token_url: Url,
        mut http_service: MockHttpService,
    ) {
        let pkce_flow = AuthorizationFlow::builder()
            .client_id(client_id)
            .authorization_url(auth_url.clone())
            .token_url(token_url.clone())
            .build();
        let scopes = vec![Scope::new("test-scope".to_string())];
        let mut mock_open_auth_url = MockOpenUrl::new();
        let mut mock_redirect_server_bind = MockRedirectServerBind::new();
        let mock_print = MockPrint::new();
        let expected_authorization_code = AuthorizationCode::new("authorizationcode".to_string());
        mock_redirect_server_bind.expect_bind().times(1).returning({
            let expected_authorization_code = expected_authorization_code.clone();
            move || {
                let mut mock_redirect_server_await = MockRedirectServerAwait::new();
                mock_redirect_server_await
                    .expect_await_response()
                    .times(1)
                    .returning({
                        let expected_authorization_code = expected_authorization_code.clone();
                        move |_| Ok(expected_authorization_code.clone())
                    });
                mock_redirect_server_await
                    .expect_local_addr()
                    .times(1)
                    .returning(|| {
                        Ok(SocketAddr::new(
                            IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
                            8080,
                        ))
                    });
                Ok(mock_redirect_server_await)
            }
        });
        let expected_auth_url = auth_url.clone();
        mock_open_auth_url
            .expect_open_url()
            .times(1)
            .withf(move |url| url.to_string().starts_with(expected_auth_url.as_str()))
            .returning(|_| Ok(()));
        let result = pkce_flow
            .authorize(
                scopes,
                &mock_open_auth_url,
                &mock_print,
                mock_redirect_server_bind,
            )
            .await;
        assert_that!(result).is_ok();
        let next = result.unwrap();
        assert_that!(next.state.code.secret()).is_equal_to(expected_authorization_code.secret());

        let expected_token_url = token_url.clone();
        http_service
            .expect_call()
            .times(1)
            .withf(move |req| {
                req.method() == Method::POST
                    && req.uri() == &Uri::try_from(expected_token_url.as_str()).unwrap()
            })
            .returning(|_| {
                let body = serde_json::json!({
                    "access_token": "access_token",
                    "refresh_token": "refresh_token",
                    "token_type": "Bearer"
                });
                let body_bytes = serde_json::to_vec(&body).unwrap();
                let body_bytes = Full::new(Bytes::from(body_bytes));
                let response = http::Response::builder().body(body_bytes).unwrap();
                futures::future::ready(Ok(response))
            });

        let result = next.exchange_code(http_service).await;
        let access_token: AccessToken = serde_json::from_str("\"access_token\"").unwrap();
        let refresh_token: RefreshToken = serde_json::from_str("\"refresh_token\"").unwrap();
        assert_that!(result)
            .is_ok()
            .is_equal_to(AuthorizationFlowResponse {
                access_token,
                refresh_token: Some(refresh_token),
                expires_in: None,
            })
    }

    #[rstest]
    #[tokio::test]
    #[timeout(Duration::from_secs(5))]
    async fn test_pkce_flow_authorize_with_open_failure(
        client_id: String,
        auth_url: Url,
        token_url: Url,
    ) {
        let pkce_flow = AuthorizationFlow::builder()
            .client_id(client_id)
            .authorization_url(auth_url.clone())
            .token_url(token_url)
            .build();
        let scopes = vec![Scope::new("test-scope".to_string())];
        let mut mock_open_auth_url = MockOpenUrl::new();
        let mut mock_redirect_server_bind = MockRedirectServerBind::new();
        let mut mock_print = MockPrint::new();
        let expected_authorization_code = AuthorizationCode::new("authorizationcode".to_string());
        mock_redirect_server_bind.expect_bind().times(1).returning({
            let expected_authorization_code = expected_authorization_code.clone();
            move || {
                let mut mock_redirect_server_await = MockRedirectServerAwait::new();
                mock_redirect_server_await
                    .expect_await_response()
                    .times(1)
                    .returning({
                        let expected_authorization_code = expected_authorization_code.clone();
                        move |_| Ok(expected_authorization_code.clone())
                    });
                mock_redirect_server_await
                    .expect_local_addr()
                    .times(1)
                    .returning(|| {
                        Ok(SocketAddr::new(
                            IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
                            8080,
                        ))
                    });
                Ok(mock_redirect_server_await)
            }
        });
        mock_open_auth_url
            .expect_open_url()
            .times(1)
            .withf({
                let auth_url = auth_url.clone();
                move |url| url.to_string().starts_with(auth_url.as_str())
            })
            .returning(|_| Err(std::io::Error::other("no")));
        mock_print
            .expect_print()
            .times(1)
            .withf(|message| {
                message.style() == &Style::Error
                    && message.text().contains(
                        "We were unable to open the OAuth Authorization URL automatically",
                    )
            })
            .returning(|_| Ok(()));
        let result = pkce_flow
            .authorize(
                scopes,
                &mock_open_auth_url,
                &mock_print,
                mock_redirect_server_bind,
            )
            .await;
        let next = assert_that!(result).is_ok().subject;
        assert_that!(next.state.code.secret()).is_equal_to(expected_authorization_code.secret());
    }
}

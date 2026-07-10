use std::{fmt::Debug, net::SocketAddr};

use oauth2::{AuthorizationCode, CsrfToken};

use super::future::{OauthRedirectError, OauthRedirectFuture};
use crate::oauth2::authorization_flow::redirect::DEFAULT_REDIRECT_HOST;

/// Errors from the OAuth redirect server.
#[derive(thiserror::Error, Debug)]
pub enum RedirectServerError {
    /// Failed to bind the TCP listener.
    #[error("Unable to bind TcpListener: {}", .0)]
    Bind(std::io::Error),
    /// Failed to retrieve the server's local address.
    #[error("Unable to fetch server address: {}", .0)]
    LocalAddr(std::io::Error),
    /// The axum redirect handler returned an error.
    #[error(transparent)]
    OauthRedirect(#[from] OauthRedirectError),
}

mod state {
    use tokio::net::TcpListener;

    #[derive(Debug)]
    pub struct AxumRedirectServerInit {}

    #[derive(Debug)]
    pub struct AxumRedirectServerBind {
        pub listener: TcpListener,
    }
}

/// A redirect server that can bind to a local port to receive the OAuth callback.
#[cfg_attr(any(test, feature = "testing"), mockall::automock(type Next = MockRedirectServerAwait;))]
#[async_trait::async_trait]
pub trait RedirectServerBind {
    /// The bound server, ready to await the OAuth callback.
    type Next: RedirectServerAwait;
    /// Binds to a random available port.
    async fn bind(self) -> Result<Self::Next, RedirectServerError>;
    /// Binds to the specified port.
    async fn bind_to(self, port: u16) -> Result<Self::Next, RedirectServerError>;
}

/// A bound redirect server waiting for the OAuth authorization callback.
#[cfg_attr(any(test, feature = "testing"), mockall::automock)]
#[async_trait::async_trait]
pub trait RedirectServerAwait {
    /// Returns the socket address the server is listening on.
    fn local_addr(&self) -> Result<SocketAddr, RedirectServerError>;
    /// Waits for the OAuth callback and returns the authorization code once the CSRF token validates.
    async fn await_response(
        self,
        csrf_token: CsrfToken,
    ) -> Result<AuthorizationCode, RedirectServerError>;
}

/// Axum-based local HTTP server that handles the OAuth redirect callback.
#[derive(Debug)]
pub struct AxumRedirectServer<T: Debug> {
    state: T,
}

impl Default for AxumRedirectServer<state::AxumRedirectServerInit> {
    fn default() -> Self {
        AxumRedirectServer {
            state: state::AxumRedirectServerInit {},
        }
    }
}

#[async_trait::async_trait]
impl RedirectServerBind for AxumRedirectServer<state::AxumRedirectServerInit> {
    type Next = AxumRedirectServer<state::AxumRedirectServerBind>;
    async fn bind(self) -> Result<Self::Next, RedirectServerError> {
        self.bind_to(0).await
    }

    async fn bind_to(self, port: u16) -> Result<Self::Next, RedirectServerError> {
        let listener = tokio::net::TcpListener::bind(format!("{}:{}", DEFAULT_REDIRECT_HOST, port))
            .await
            .map_err(RedirectServerError::Bind)?;
        Ok(AxumRedirectServer {
            state: state::AxumRedirectServerBind { listener },
        })
    }
}

#[async_trait::async_trait]
impl RedirectServerAwait for AxumRedirectServer<state::AxumRedirectServerBind> {
    fn local_addr(&self) -> Result<SocketAddr, RedirectServerError> {
        self.state
            .listener
            .local_addr()
            .map_err(RedirectServerError::LocalAddr)
    }
    async fn await_response(
        self,
        csrf_token: CsrfToken,
    ) -> Result<AuthorizationCode, RedirectServerError> {
        OauthRedirectFuture::new(self.state.listener, csrf_token)
            .await
            .map_err(RedirectServerError::from)
    }
}

#[cfg(test)]
mod tests {
    use std::{
        net::{IpAddr, Ipv4Addr},
        time::Duration,
    };

    use oauth2::CsrfToken;
    use rstest::rstest;
    use speculoos::prelude::*;
    use tokio::net::TcpListener;

    use super::{
        AxumRedirectServer,
        state::{AxumRedirectServerBind, AxumRedirectServerInit},
    };
    use crate::oauth2::authorization_flow::redirect::server::{
        RedirectServerAwait, RedirectServerBind,
    };

    #[tokio::test]
    async fn test_bind() {
        let redirect_server = AxumRedirectServer {
            state: AxumRedirectServerInit {},
        };
        let result = redirect_server.bind().await;
        let subject = assert_that!(result).is_ok().subject;
        let socket_addr_result = subject.local_addr();
        let socket_addr = assert_that!(socket_addr_result).is_ok().subject;
        assert_that!(socket_addr.ip()).is_equal_to(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)));
        assert_that!(socket_addr.port()).is_not_equal_to(0);
    }

    #[tokio::test]
    async fn test_bind_to() {
        let redirect_server = AxumRedirectServer {
            state: AxumRedirectServerInit {},
        };
        let result = redirect_server.bind_to(4000).await;
        let subject = assert_that!(result).is_ok().subject;
        let socket_addr_result = subject.local_addr();
        let socket_addr = assert_that!(socket_addr_result).is_ok().subject;
        assert_that!(socket_addr.ip()).is_equal_to(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)));
        assert_that!(socket_addr.port()).is_not_equal_to(0);

        let another_server = AxumRedirectServer {
            state: AxumRedirectServerInit {},
        };
        let result = another_server.bind_to(4000).await;
        assert_that!(result).is_err();
    }

    #[rstest]
    #[tokio::test]
    #[timeout(Duration::from_secs(5))]
    async fn test_await_response_success() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let authorization_code = "authorization".to_string();
        let csrf_token = CsrfToken::new_random();
        let await_response = tokio::task::spawn({
            let csrf_token = csrf_token.clone();
            let redirect_server = AxumRedirectServer {
                state: AxumRedirectServerBind { listener },
            };
            async move { redirect_server.await_response(csrf_token.clone()).await }
        });

        let url = format!(
            "http://{}:{}/?code={}&state={}",
            addr.ip(),
            addr.port(),
            authorization_code,
            csrf_token.secret()
        );
        let result = reqwest::get(&url).await;
        let resp = assert_that!(result).is_ok().subject;
        assert_that!(resp.status().as_u16()).is_equal_to(200u16);

        let await_response = await_response.await.unwrap();
        let actual_authorization_code = assert_that!(await_response).is_ok().subject;
        assert_that!(actual_authorization_code.secret()).is_equal_to(&authorization_code);
    }

    #[rstest]
    #[tokio::test]
    #[timeout(Duration::from_secs(5))]
    async fn test_await_response_bad_csrf() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let authorization_code = "authorization".to_string();
        let csrf_token = CsrfToken::new_random();
        let _server_handle = tokio::task::spawn({
            let csrf_token = csrf_token.clone();
            let redirect_server = AxumRedirectServer {
                state: AxumRedirectServerBind { listener },
            };
            async move { redirect_server.await_response(csrf_token.clone()).await }
        });

        let url = format!(
            "http://{}:{}/?code={}&state={}",
            addr.ip(),
            addr.port(),
            authorization_code,
            "bad"
        );
        let resp = reqwest::get(&url).await.unwrap();
        assert_that!(resp.status().as_u16()).is_equal_to(400u16);
        let body = resp.text().await.unwrap();
        assert_that!(body.as_str()).is_equal_to("Invalid CSRF token");
    }
}

use std::{fmt::Debug, time::Duration};

use oauth2::{
    AccessToken, ClientId, DeviceAuthorizationUrl, DeviceCodeErrorResponseType, EndpointNotSet,
    EndpointSet, RefreshToken, RequestTokenError, Scope, StandardDeviceAuthorizationResponse,
    TokenResponse, TokenUrl, basic::BasicClient,
};
use rover_http::Body;
use rover_print::{
    print::Print,
    style::{Style, StyledText},
};
use tower::Service;
use url::Url;

use crate::{OauthHttpClient, oauth2::OauthTokens};

type DeviceAuthorizationFlowClient =
    BasicClient<EndpointNotSet, EndpointSet, EndpointNotSet, EndpointNotSet, EndpointSet>;

mod state;

/// Errors from the RFC 8628 device authorization grant.
#[derive(thiserror::Error, Debug)]
pub enum DeviceAuthorizationFlowError {
    /// The device authorization endpoint rejected the device code request.
    #[error("Failed to request a device code")]
    DeviceCodeRequest(#[source] Box<dyn std::error::Error>),
    /// The user denied the authorization request.
    #[error("The authorization request was denied")]
    AccessDenied,
    /// The device code expired before the user completed authorization.
    #[error("The device code expired before authorization completed")]
    ExpiredToken,
    /// Polling the token endpoint failed for any other reason (a transport
    /// failure, or another server error).
    #[error("Failed to obtain an access token")]
    AccessTokenPoll(#[source] Box<dyn std::error::Error>),
}

/// State machine for the OAuth2 RFC 8628 device authorization grant.
#[derive(Debug)]
pub struct DeviceAuthorizationFlow<T>
where
    T: Debug + state::DeviceAuthorizationFlowState,
{
    state: T,
}

#[bon::bon]
impl DeviceAuthorizationFlow<state::DeviceAuthorizationFlowInit> {
    #[builder]
    /// Creates a new [`DeviceAuthorizationFlow`] in its initial state.
    pub const fn new(
        client_id: String,
        device_authorization_url: Url,
        token_url: Url,
    ) -> DeviceAuthorizationFlow<state::DeviceAuthorizationFlowInit> {
        DeviceAuthorizationFlow {
            state: state::DeviceAuthorizationFlowInit {
                client_id,
                device_authorization_url,
                token_url,
            },
        }
    }

    /// Requests a device code and user code from the device authorization
    /// endpoint, then prints the verification URL and code the user must
    /// visit. Unlike the PKCE flow, no browser is opened here - that's the
    /// whole point of `--no-browser`, the user is expected to complete
    /// verification from any browser/device they choose.
    pub async fn request_device_code<S, B, P>(
        &self,
        scopes: Vec<Scope>,
        http_service: S,
        stderr: &P,
    ) -> Result<
        DeviceAuthorizationFlow<state::DeviceAuthorizationFlowWithDeviceCode>,
        DeviceAuthorizationFlowError,
    >
    where
        S: Service<http::Request<B>, Response = http::Response<B>> + Send + 'static,
        S::Error: std::error::Error + Send + Sync + From<B::Error> + 'static,
        S::Future: Send,
        B: From<Vec<u8>> + Body + Unpin + Send,
        B::Data: Send,
        P: Print,
    {
        let client = BasicClient::new(ClientId::new(self.state.client_id.clone()))
            .set_device_authorization_url(DeviceAuthorizationUrl::from_url(
                self.state.device_authorization_url.clone(),
            ))
            .set_token_uri(TokenUrl::from_url(self.state.token_url.clone()));
        let http_client = OauthHttpClient::new(http_service);
        let device_auth_response: StandardDeviceAuthorizationResponse = client
            .exchange_device_code()
            .add_scopes(scopes)
            .request_async(&http_client)
            .await
            .map_err(|err| DeviceAuthorizationFlowError::DeviceCodeRequest(Box::new(err)))?;

        stderr.print_line(&[
            StyledText::plain("To finish logging in, visit "),
            StyledText::new(
                Style::Link,
                device_auth_response.verification_uri().to_string(),
            ),
            StyledText::plain(" and enter the code: "),
            StyledText::new(
                Style::Command,
                device_auth_response.user_code().secret().clone(),
            ),
        ]);
        if let Some(complete_uri) = device_auth_response.verification_uri_complete() {
            stderr.print_line(&[
                StyledText::plain("Or open this URL directly: "),
                StyledText::new(Style::Link, complete_uri.secret().clone()),
            ]);
        }

        Ok(DeviceAuthorizationFlow {
            state: state::DeviceAuthorizationFlowWithDeviceCode {
                client,
                device_auth_response,
            },
        })
    }
}

impl DeviceAuthorizationFlow<state::DeviceAuthorizationFlowWithDeviceCode> {
    /// The URL the user should visit to enter their user code.
    pub fn verification_uri(&self) -> &EndUserVerificationUrl {
        self.state.device_auth_response.verification_uri()
    }

    /// The URL the user should visit, with the user code already embedded
    /// (if the server provided one), so no separate code entry is required.
    pub fn verification_uri_complete(&self) -> Option<&VerificationUriComplete> {
        self.state.device_auth_response.verification_uri_complete()
    }

    /// The code the user must enter at [`Self::verification_uri`].
    pub fn user_code(&self) -> &str {
        self.state.device_auth_response.user_code().secret()
    }

    /// How long the device code remains valid for.
    pub fn expires_in(&self) -> Duration {
        self.state.device_auth_response.expires_in()
    }

    /// Polls the token endpoint per RFC 8628 §3.5 until the user authorizes,
    /// denies, the device code expires, or `poll_timeout` elapses (if given).
    /// All poll-interval/backoff timing is handled internally by the
    /// `oauth2` crate's own request loop, so this doesn't reimplement any of
    /// it. `poll_timeout: None` polls until the server's own declared
    /// `expires_in`, matching the RFC's default behavior.
    pub async fn poll_for_token<S, B>(
        &self,
        http_service: S,
        poll_timeout: Option<Duration>,
    ) -> Result<OauthTokens, DeviceAuthorizationFlowError>
    where
        S: Service<http::Request<B>, Response = http::Response<B>> + Send + 'static,
        S::Error: std::error::Error + Send + Sync + From<B::Error> + 'static,
        S::Future: Send,
        B: From<Vec<u8>> + Body + Unpin + Send,
        B::Data: Send,
    {
        let http_client = OauthHttpClient::new(http_service);
        let resp = self
            .state
            .client
            .exchange_device_access_token(&self.state.device_auth_response)
            .request_async(&http_client, tokio::time::sleep, poll_timeout)
            .await
            .map_err(|err| {
                let error_type = match &err {
                    RequestTokenError::ServerResponse(resp) => Some(resp.error().clone()),
                    _ => None,
                };
                match error_type {
                    Some(DeviceCodeErrorResponseType::AccessDenied) => {
                        DeviceAuthorizationFlowError::AccessDenied
                    }
                    Some(DeviceCodeErrorResponseType::ExpiredToken) => {
                        DeviceAuthorizationFlowError::ExpiredToken
                    }
                    _ => DeviceAuthorizationFlowError::AccessTokenPoll(Box::new(err)),
                }
            })?;

        let access_token: AccessToken = resp.access_token().clone();
        let refresh_token: Option<RefreshToken> = resp.refresh_token().cloned();
        let expires_in = resp.expires_in();
        Ok(OauthTokens {
            access_token,
            refresh_token,
            expires_in,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, time::Duration};

    use bytes::Bytes;
    use http::{Method, Uri};
    use oauth2::{AccessToken, RefreshToken, Scope};
    use rover_http::{BodyExt, Full, HttpServiceError, test::MockHttpService};
    use rover_print::{print::MockPrint, style::Style};
    use rstest::{fixture, rstest};
    use speculoos::prelude::*;
    use url::{Url, form_urlencoded};

    use crate::oauth2::{
        OauthTokens,
        device_authorization_flow::{DeviceAuthorizationFlow, DeviceAuthorizationFlowError},
    };

    #[fixture]
    fn device_authorization_url() -> Url {
        Url::parse("https://example.com/device/authorize").unwrap()
    }

    #[fixture]
    fn token_url() -> Url {
        Url::parse("https://example.com/token").unwrap()
    }

    #[fixture]
    fn client_id() -> String {
        "client_id".to_string()
    }

    fn device_auth_response_body() -> serde_json::Value {
        serde_json::json!({
            "device_code": "the-device-code",
            "user_code": "ABCD-EFGH",
            "verification_uri": "https://example.com/device",
            "verification_uri_complete": "https://example.com/device?user_code=ABCD-EFGH",
            "expires_in": 600,
            "interval": 1
        })
    }

    /// Decodes a mocked request's form-encoded body into its key/value
    /// pairs, so tests can assert on the actual wire contract instead of
    /// only the method/URI. `Full<Bytes>` is already fully buffered, so
    /// collecting it never actually awaits I/O.
    fn form_body(req: &http::Request<Full<Bytes>>) -> HashMap<String, String> {
        let bytes = futures::executor::block_on(req.body().clone().collect())
            .unwrap()
            .to_bytes();
        form_urlencoded::parse(&bytes).into_owned().collect()
    }

    #[rstest]
    #[tokio::test]
    #[timeout(Duration::from_secs(5))]
    async fn request_device_code_prints_the_verification_url_and_code(
        client_id: String,
        device_authorization_url: Url,
        token_url: Url,
    ) {
        let flow = DeviceAuthorizationFlow::builder()
            .client_id(client_id.clone())
            .device_authorization_url(device_authorization_url.clone())
            .token_url(token_url)
            .build();

        let mut http_service = MockHttpService::new();
        let expected_device_authorization_url = device_authorization_url.clone();
        http_service
            .expect_call()
            .times(1)
            .withf(move |req| {
                req.method() == Method::POST
                    && req.uri()
                        == &Uri::try_from(expected_device_authorization_url.as_str()).unwrap()
                    && form_body(req)
                        == HashMap::from([
                            ("client_id".to_string(), client_id.clone()),
                            ("scope".to_string(), "openid".to_string()),
                        ])
            })
            .returning(|_| {
                let body_bytes = serde_json::to_vec(&device_auth_response_body()).unwrap();
                let response = http::Response::builder()
                    .body(Full::new(Bytes::from(body_bytes)))
                    .unwrap();
                futures::future::ready(Ok(response))
            });

        let mut mock_print = MockPrint::new();
        mock_print
            .expect_print_line()
            .times(1)
            .withf(|segments| {
                let link = segments.iter().find(|s| s.style() == &Style::Link);
                let command = segments.iter().find(|s| s.style() == &Style::Command);
                link.is_some_and(|s| s.text() == "https://example.com/device")
                    && command.is_some_and(|s| s.text() == "ABCD-EFGH")
            })
            .returning(|_| ());
        mock_print
            .expect_print_line()
            .times(1)
            .withf(|segments| {
                segments.iter().any(|s| {
                    s.style() == &Style::Link
                        && s.text() == "https://example.com/device?user_code=ABCD-EFGH"
                })
            })
            .returning(|_| ());

        let result = flow
            .request_device_code(
                vec![Scope::new("openid".to_string())],
                http_service,
                &mock_print,
            )
            .await;

        let next = assert_that!(result).is_ok().subject;
        assert_that!(next.state.device_auth_response.user_code().secret())
            .is_equal_to(&"ABCD-EFGH".to_string());
    }

    #[rstest]
    #[tokio::test]
    #[timeout(Duration::from_secs(5))]
    async fn request_device_code_fails_when_the_server_rejects_the_request(
        client_id: String,
        device_authorization_url: Url,
        token_url: Url,
    ) {
        let flow = DeviceAuthorizationFlow::builder()
            .client_id(client_id)
            .device_authorization_url(device_authorization_url)
            .token_url(token_url)
            .build();

        let mut http_service = MockHttpService::new();
        http_service.expect_call().times(1).returning(|_| {
            let response = http::Response::builder()
                .status(http::StatusCode::BAD_REQUEST)
                .body(Full::new(Bytes::from_static(
                    b"{\"error\":\"invalid_client\"}",
                )))
                .unwrap();
            futures::future::ready(Ok(response))
        });
        let mock_print = MockPrint::new();

        let result = flow
            .request_device_code(Vec::new(), http_service, &mock_print)
            .await;

        assert_that!(result)
            .is_err()
            .matches(|err| matches!(err, DeviceAuthorizationFlowError::DeviceCodeRequest(_)));
    }

    #[rstest]
    #[tokio::test]
    #[timeout(Duration::from_secs(5))]
    async fn request_device_code_fails_when_the_transport_fails(
        client_id: String,
        device_authorization_url: Url,
        token_url: Url,
    ) {
        let flow = DeviceAuthorizationFlow::builder()
            .client_id(client_id)
            .device_authorization_url(device_authorization_url)
            .token_url(token_url)
            .build();

        let mut http_service = MockHttpService::new();
        http_service
            .expect_call()
            .times(1)
            .returning(|_| futures::future::ready(Err(HttpServiceError::TimedOut)));
        let mock_print = MockPrint::new();

        let result = flow
            .request_device_code(Vec::new(), http_service, &mock_print)
            .await;

        assert_that!(result)
            .is_err()
            .matches(|err| matches!(err, DeviceAuthorizationFlowError::DeviceCodeRequest(_)));
    }

    #[rstest]
    #[tokio::test]
    #[timeout(Duration::from_secs(5))]
    async fn poll_for_token_succeeds_once_the_user_approves(
        client_id: String,
        device_authorization_url: Url,
        token_url: Url,
    ) {
        let expected_client_id = client_id.clone();
        let flow = DeviceAuthorizationFlow::builder()
            .client_id(client_id)
            .device_authorization_url(device_authorization_url)
            .token_url(token_url.clone())
            .build();

        let mut device_code_service = MockHttpService::new();
        device_code_service.expect_call().times(1).returning(|_| {
            let body_bytes = serde_json::to_vec(&device_auth_response_body()).unwrap();
            let response = http::Response::builder()
                .body(Full::new(Bytes::from(body_bytes)))
                .unwrap();
            futures::future::ready(Ok(response))
        });
        let mut mock_print = MockPrint::new();
        mock_print.expect_print_line().returning(|_| ());
        let with_device_code = flow
            .request_device_code(Vec::new(), device_code_service, &mock_print)
            .await
            .unwrap();

        let mut token_service = MockHttpService::new();
        let expected_token_url = token_url.clone();
        token_service
            .expect_call()
            .times(1)
            .withf(move |req| {
                req.method() == Method::POST
                    && req.uri() == &Uri::try_from(expected_token_url.as_str()).unwrap()
                    && form_body(req)
                        == HashMap::from([
                            (
                                "grant_type".to_string(),
                                "urn:ietf:params:oauth:grant-type:device_code".to_string(),
                            ),
                            ("device_code".to_string(), "the-device-code".to_string()),
                            ("client_id".to_string(), expected_client_id.clone()),
                        ])
            })
            .returning(|_| {
                let body = serde_json::json!({
                    "access_token": "access_token",
                    "refresh_token": "refresh_token",
                    "token_type": "Bearer"
                });
                let body_bytes = serde_json::to_vec(&body).unwrap();
                let response = http::Response::builder()
                    .body(Full::new(Bytes::from(body_bytes)))
                    .unwrap();
                futures::future::ready(Ok(response))
            });

        let result = with_device_code.poll_for_token(token_service, None).await;

        let access_token: AccessToken = serde_json::from_str("\"access_token\"").unwrap();
        let refresh_token: RefreshToken = serde_json::from_str("\"refresh_token\"").unwrap();
        assert_that!(result).is_ok().is_equal_to(OauthTokens {
            access_token,
            refresh_token: Some(refresh_token),
            expires_in: None,
        });
    }

    #[rstest]
    #[tokio::test]
    #[timeout(Duration::from_secs(5))]
    async fn poll_for_token_fails_when_the_user_denies_access(
        client_id: String,
        device_authorization_url: Url,
        token_url: Url,
    ) {
        let flow = DeviceAuthorizationFlow::builder()
            .client_id(client_id)
            .device_authorization_url(device_authorization_url)
            .token_url(token_url)
            .build();

        let mut device_code_service = MockHttpService::new();
        device_code_service.expect_call().times(1).returning(|_| {
            let body_bytes = serde_json::to_vec(&device_auth_response_body()).unwrap();
            let response = http::Response::builder()
                .body(Full::new(Bytes::from(body_bytes)))
                .unwrap();
            futures::future::ready(Ok(response))
        });
        let mut mock_print = MockPrint::new();
        mock_print.expect_print_line().returning(|_| ());
        let with_device_code = flow
            .request_device_code(Vec::new(), device_code_service, &mock_print)
            .await
            .unwrap();

        let mut token_service = MockHttpService::new();
        token_service.expect_call().times(1).returning(|_| {
            let response = http::Response::builder()
                .status(http::StatusCode::BAD_REQUEST)
                .body(Full::new(Bytes::from_static(
                    b"{\"error\":\"access_denied\",\"error_description\":\"Access Denied\"}",
                )))
                .unwrap();
            futures::future::ready(Ok(response))
        });

        let result = with_device_code.poll_for_token(token_service, None).await;

        assert_that!(result)
            .is_err()
            .matches(|err| matches!(err, DeviceAuthorizationFlowError::AccessDenied));
    }

    #[rstest]
    #[tokio::test]
    #[timeout(Duration::from_secs(5))]
    async fn poll_for_token_fails_when_the_device_code_expires(
        client_id: String,
        device_authorization_url: Url,
        token_url: Url,
    ) {
        let flow = DeviceAuthorizationFlow::builder()
            .client_id(client_id)
            .device_authorization_url(device_authorization_url)
            .token_url(token_url)
            .build();

        let mut device_code_service = MockHttpService::new();
        device_code_service.expect_call().times(1).returning(|_| {
            let body_bytes = serde_json::to_vec(&device_auth_response_body()).unwrap();
            let response = http::Response::builder()
                .body(Full::new(Bytes::from(body_bytes)))
                .unwrap();
            futures::future::ready(Ok(response))
        });
        let mut mock_print = MockPrint::new();
        mock_print.expect_print_line().returning(|_| ());
        let with_device_code = flow
            .request_device_code(Vec::new(), device_code_service, &mock_print)
            .await
            .unwrap();

        let mut token_service = MockHttpService::new();
        token_service.expect_call().times(1).returning(|_| {
            let response = http::Response::builder()
                .status(http::StatusCode::BAD_REQUEST)
                .body(Full::new(Bytes::from_static(
                    b"{\"error\":\"expired_token\"}",
                )))
                .unwrap();
            futures::future::ready(Ok(response))
        });

        let result = with_device_code.poll_for_token(token_service, None).await;

        assert_that!(result)
            .is_err()
            .matches(|err| matches!(err, DeviceAuthorizationFlowError::ExpiredToken));
    }

    #[rstest]
    #[tokio::test(start_paused = true)]
    #[timeout(Duration::from_secs(5))]
    async fn poll_for_token_retries_while_authorization_is_pending(
        client_id: String,
        device_authorization_url: Url,
        token_url: Url,
    ) {
        let flow = DeviceAuthorizationFlow::builder()
            .client_id(client_id)
            .device_authorization_url(device_authorization_url)
            .token_url(token_url)
            .build();

        let mut device_code_service = MockHttpService::new();
        device_code_service.expect_call().times(1).returning(|_| {
            let body_bytes = serde_json::to_vec(&device_auth_response_body()).unwrap();
            let response = http::Response::builder()
                .body(Full::new(Bytes::from(body_bytes)))
                .unwrap();
            futures::future::ready(Ok(response))
        });
        let mut mock_print = MockPrint::new();
        mock_print.expect_print_line().returning(|_| ());
        let with_device_code = flow
            .request_device_code(Vec::new(), device_code_service, &mock_print)
            .await
            .unwrap();

        let mut token_service = MockHttpService::new();
        let mut calls = 0;
        token_service.expect_call().times(2).returning(move |_| {
            calls += 1;
            let response = if calls == 1 {
                http::Response::builder()
                    .status(http::StatusCode::BAD_REQUEST)
                    .body(Full::new(Bytes::from_static(
                        b"{\"error\":\"authorization_pending\"}",
                    )))
                    .unwrap()
            } else {
                let body = serde_json::json!({
                    "access_token": "access_token",
                    "refresh_token": "refresh_token",
                    "token_type": "Bearer"
                });
                http::Response::builder()
                    .body(Full::new(Bytes::from(serde_json::to_vec(&body).unwrap())))
                    .unwrap()
            };
            futures::future::ready(Ok(response))
        });

        let start = tokio::time::Instant::now();
        let result = with_device_code.poll_for_token(token_service, None).await;

        assert_that!(start.elapsed()).matches(|elapsed| *elapsed >= Duration::from_secs(1));

        let access_token: AccessToken = serde_json::from_str("\"access_token\"").unwrap();
        let refresh_token: RefreshToken = serde_json::from_str("\"refresh_token\"").unwrap();
        assert_that!(result).is_ok().is_equal_to(OauthTokens {
            access_token,
            refresh_token: Some(refresh_token),
            expires_in: None,
        });
    }
}

use std::fmt::Debug;

use oauth2::{
    AccessToken, ClientId, DeviceAuthorizationUrl, EndpointNotSet, EndpointSet, RefreshToken,
    Scope, StandardDeviceAuthorizationResponse, TokenResponse, TokenUrl, basic::BasicClient,
};
use rover_http::Body;
use rover_print::{
    print::Print,
    style::{Style, StyledText},
};
use tower::Service;
use url::Url;

use crate::{OauthHttpClient, oauth2::authorization_flow::AuthorizationFlowResponse};

type DeviceAuthorizationFlowClient =
    BasicClient<EndpointNotSet, EndpointSet, EndpointNotSet, EndpointNotSet, EndpointSet>;

mod state;

/// Errors from the RFC 8628 device authorization grant.
#[derive(thiserror::Error, Debug)]
pub enum DeviceAuthorizationFlowError {
    /// The device authorization endpoint rejected the device code request.
    #[error("Failed to request a device code: {}", .0)]
    DeviceCodeRequest(Box<dyn std::error::Error>),
    /// Polling the token endpoint did not end in a token (denied, expired, or another server error).
    #[error("Failed to obtain an access token: {}", .0)]
    AccessTokenPoll(Box<dyn std::error::Error>),
}

/// State machine for the OAuth2 RFC 8628 device authorization grant.
#[derive(Debug)]
pub struct DeviceAuthorizationFlow<T>
where
    T: Debug,
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
        S::Error: std::error::Error + From<B::Error> + 'static,
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
            stderr.print(&StyledText::new(
                Style::Info,
                format!("Or open this URL directly: {}", complete_uri.secret()),
            ));
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
    /// Polls the token endpoint per RFC 8628 §3.5 until the user authorizes,
    /// denies, or the device code expires. All poll-interval/backoff timing
    /// is handled internally by the `oauth2` crate's own request loop, so
    /// this doesn't reimplement any of it.
    pub async fn poll_for_token<S, B>(
        self,
        http_service: S,
    ) -> Result<AuthorizationFlowResponse, DeviceAuthorizationFlowError>
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
            .exchange_device_access_token(&self.state.device_auth_response)
            .request_async(&http_client, tokio::time::sleep, None)
            .await
            .map_err(|err| DeviceAuthorizationFlowError::AccessTokenPoll(Box::new(err)))?;

        let access_token: AccessToken = resp.access_token().clone();
        let refresh_token: Option<RefreshToken> = resp.refresh_token().cloned();
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
    use std::time::Duration;

    use bytes::Bytes;
    use http::{Method, Uri};
    use oauth2::{AccessToken, RefreshToken};
    use rover_http::{Full, test::MockHttpService};
    use rover_print::print::MockPrint;
    use rstest::{fixture, rstest};
    use speculoos::prelude::*;
    use url::Url;

    use crate::oauth2::{
        authorization_flow::AuthorizationFlowResponse,
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

    #[rstest]
    #[tokio::test]
    #[timeout(Duration::from_secs(5))]
    async fn request_device_code_prints_the_verification_url_and_code(
        client_id: String,
        device_authorization_url: Url,
        token_url: Url,
    ) {
        let flow = DeviceAuthorizationFlow::builder()
            .client_id(client_id)
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
                let text: String = segments.iter().map(|s| s.text()).collect();
                text.contains("https://example.com/device") && text.contains("ABCD-EFGH")
            })
            .returning(|_| ());
        mock_print
            .expect_print()
            .times(1)
            .withf(|message| {
                message
                    .text()
                    .contains("https://example.com/device?user_code=ABCD-EFGH")
            })
            .returning(|_| ());

        let result = flow
            .request_device_code(Vec::new(), http_service, &mock_print)
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
    async fn poll_for_token_succeeds_once_the_user_approves(
        client_id: String,
        device_authorization_url: Url,
        token_url: Url,
    ) {
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
        mock_print.expect_print().returning(|_| ());
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

        let result = with_device_code.poll_for_token(token_service).await;

        let access_token: AccessToken = serde_json::from_str("\"access_token\"").unwrap();
        let refresh_token: RefreshToken = serde_json::from_str("\"refresh_token\"").unwrap();
        assert_that!(result)
            .is_ok()
            .is_equal_to(AuthorizationFlowResponse {
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
        mock_print.expect_print().returning(|_| ());
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

        let result = with_device_code.poll_for_token(token_service).await;

        assert_that!(result)
            .is_err()
            .matches(|err| matches!(err, DeviceAuthorizationFlowError::AccessTokenPoll(_)));
    }
}

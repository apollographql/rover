use bytes::Bytes;
use http::Method;
use oauth2::Scope;
use rover_http::{Full, HttpRequest, HttpResponse, body::body_to_bytes};
use rover_tower::{ResponseFuture, service::replace_ready_service};
use serde::{Deserialize, Serialize};
use tower::Service;
use url::Url;

use super::{GrantType, TokenEndpointAuthMethod};

/// Request body for OAuth2 dynamic client registration.
#[derive(Clone, Debug, Serialize)]
#[serde(rename = "snake_case")]
pub struct RegisterRequest {
    #[serde(skip)]
    register_url: Url,
    scopes: Vec<Scope>,
    redirect_uris: Vec<Url>,
    grant_types: Vec<GrantType>,
    token_endpoint_auth_method: TokenEndpointAuthMethod,
}

#[bon::bon]
impl RegisterRequest {
    /// Creates a new [`RegisterRequest`].
    #[builder]
    pub fn new(register_url: Url, redirect_url: Url) -> RegisterRequest {
        RegisterRequest {
            register_url,
            scopes: vec![
                Scope::new("rover:cli".to_string()),
                Scope::new("openid".to_string()),
                Scope::new("profile".to_string()),
                Scope::new("email".to_string()),
            ],
            redirect_uris: vec![redirect_url],
            grant_types: vec![GrantType::AuthorizationCode],
            token_endpoint_auth_method: TokenEndpointAuthMethod::None,
        }
    }
}

/// Errors from the [`Register`] service.
#[derive(thiserror::Error, Debug)]
pub enum RegisterError {
    /// HTTP transport error.
    #[error(transparent)]
    Http(Box<dyn std::error::Error + Send>),
    /// Failed to deserialize the server response.
    #[error("Failed to deserialize response: {}.", .0)]
    Deserialize(serde_json::Error),
    /// Failed to serialize the registration request.
    #[error("Failed to serialize request: {}.", .0)]
    Serialize(serde_json::Error),
}

/// Successful response from dynamic client registration.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename = "snake_case")]
pub struct RegisterResponse {
    /// The registered client ID.
    pub client_id: String,
}

/// Tower service for OAuth2 dynamic client registration (RFC 7591).
pub struct Register<S> {
    inner: S,
}

impl<S> Register<S> {
    /// Creates a new [`Register`] wrapping the given HTTP service.
    pub const fn new(inner: S) -> Register<S> {
        Register { inner }
    }
}

impl<S> Service<RegisterRequest> for Register<S>
where
    S: Service<HttpRequest, Response = HttpResponse> + Clone + Send + 'static,
    S::Error: std::error::Error + Send + 'static,
    S::Future: Send,
{
    type Response = RegisterResponse;
    type Error = RegisterError;
    type Future = ResponseFuture<Result<Self::Response, Self::Error>>;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner
            .poll_ready(cx)
            .map_err(|err| RegisterError::Http(Box::new(err)))
    }

    fn call(&mut self, req: RegisterRequest) -> Self::Future {
        let mut inner = replace_ready_service(&mut self.inner);
        let fut = async move {
            let body = serde_json::to_vec(&req).map_err(RegisterError::Serialize)?;
            let body = Full::new(Bytes::copy_from_slice(&body));
            let req = http::Request::builder()
                .uri(req.register_url.as_str())
                .method(Method::POST)
                .header(
                    http::header::ACCEPT,
                    http::HeaderValue::from_static("application/json"),
                )
                .header(
                    http::header::CONTENT_TYPE,
                    http::HeaderValue::from_static("application/json"),
                )
                .body(body)
                .map_err(|err| RegisterError::Http(Box::new(err)))?;
            let mut resp = inner
                .call(req)
                .await
                .map_err(|err| RegisterError::Http(Box::new(err)))?;

            if !resp.status().is_success() {
                return Err(RegisterError::Http(Box::new(std::io::Error::other(
                    format!("unexpected HTTP status: {}", resp.status()),
                ))));
            }

            let body = body_to_bytes(resp.body_mut())
                .await
                .map_err(|err| RegisterError::Http(Box::new(err)))?;
            let resp = serde_json::from_slice(&body).map_err(RegisterError::Deserialize)?;
            Ok(resp)
        };
        Box::pin(fut)
    }
}

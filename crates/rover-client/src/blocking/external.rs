//! External-token response privacy and opt-in identity token capture.

use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context, Poll},
};

use rover_auth::external_token::ExternalAccessToken;
use rover_http::{BodyExt, HttpRequest, HttpResponse, HttpServiceError};
use serde::Deserialize;
use tower::Service;

/// Opt-in, short-lived capture for deliberate identity credential unmasking.
///
/// Use a fresh capture for each identity lookup, and inspect it only after that lookup
/// succeeds. It observes reads made by authentication, never supplies credentials to it,
/// and does not reread the file after the server has answered.
/// Clones share one slot; never share a capture between overlapping lookups.
#[derive(Clone, Default)]
pub struct ExternalTokenCapture(Arc<Mutex<Option<ExternalAccessToken>>>);

impl std::fmt::Debug for ExternalTokenCapture {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ExternalTokenCapture([REDACTED])")
    }
}

impl ExternalTokenCapture {
    /// Removes the last observed token, which authenticates the final attempt on success.
    ///
    /// The caller must check that its lookup succeeded before calling this: observing a
    /// source read does not prove the server accepted the token. Stored-credential services
    /// do not populate the capture.
    pub fn take(&self) -> Option<ExternalAccessToken> {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take()
    }

    pub(super) fn observe(&self, token: ExternalAccessToken) -> ExternalAccessToken {
        *self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(token.clone());
        token
    }
}

/// Recognizes response errors without allocating or deserializing the response's data.
#[derive(Deserialize)]
struct Errors {
    errors: Option<Vec<graphql_client::Error>>,
}

/// Redacts an external token that the server reflected into a GraphQL error.
///
/// Wraps each authenticated attempt, which leaves the attempt's token on the response.
/// Successful response data is never rewritten.
#[derive(Clone)]
pub(super) struct RedactReflectedToken<S> {
    inner: S,
}

impl<S> RedactReflectedToken<S> {
    pub(super) const fn new(inner: S) -> Self {
        Self { inner }
    }

    fn redact(value: &mut serde_json::Value, token: &ExternalAccessToken) {
        match value {
            serde_json::Value::String(value) => {
                *value = value.replace(token.secret(), "[REDACTED]");
            }
            serde_json::Value::Array(values) => {
                values
                    .iter_mut()
                    .for_each(|value| Self::redact(value, token));
            }
            serde_json::Value::Object(values) => {
                *values = std::mem::take(values)
                    .into_iter()
                    .map(|(key, mut value)| {
                        Self::redact(&mut value, token);
                        (key.replace(token.secret(), "[REDACTED]"), value)
                    })
                    .collect();
            }
            _ => {}
        }
    }
}

impl<S> Service<HttpRequest> for RedactReflectedToken<S>
where
    S: Service<HttpRequest, Response = HttpResponse, Error = HttpServiceError>,
    S::Future: Send + 'static,
{
    type Response = HttpResponse;
    type Error = HttpServiceError;
    type Future = Pin<Box<dyn Future<Output = Result<HttpResponse, HttpServiceError>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, request: HttpRequest) -> Self::Future {
        let future = self.inner.call(request);
        Box::pin(async move {
            let mut response = future.await?;
            let Some(token) = response.extensions_mut().remove::<ExternalAccessToken>() else {
                return Ok(response);
            };
            let Ok(collected) = response.body().clone().collect().await;
            let bytes = collected.to_bytes();
            let Some(errors) = serde_json::from_slice::<Errors>(&bytes)
                .ok()
                .and_then(|body| body.errors)
            else {
                return Ok(response);
            };
            let encoded = serde_json::to_string(&errors)
                .map_err(|error| HttpServiceError::Unexpected(Box::new(error)))?;
            if encoded.contains(token.secret()) {
                // Parse data only in the exceptional case of a reflected credential.
                // serde_json's normal depth limit also bounds the redaction traversal.
                let mut body: serde_json::Value = serde_json::from_slice(&bytes)
                    .map_err(|error| HttpServiceError::Unexpected(Box::new(error)))?;
                if let Some(errors) = body.get_mut("errors") {
                    Self::redact(errors, &token);
                }
                let bytes = serde_json::to_vec(&body)
                    .map_err(|error| HttpServiceError::Unexpected(Box::new(error)))?;
                *response.body_mut() = http_body_util::Full::new(bytes::Bytes::from(bytes));
                response.headers_mut().remove(http::header::CONTENT_LENGTH);
            }
            Ok(response)
        })
    }
}

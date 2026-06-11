use std::convert::Infallible;

use axum::{
    extract::{Query, Request, rejection::QueryRejection},
    response::{IntoResponse, Response},
};
use bon::Builder;
use http::StatusCode;
use oauth2::CsrfToken;
use rover_tower::ResponseFuture;
use serde::Deserialize;
use tower::Service;

use super::OneshotSender;

#[derive(Debug, Deserialize)]
struct QueryParams {
    code: String,
    state: CsrfToken,
}

#[derive(thiserror::Error, Debug)]
enum HandlerError {
    #[error("Invalid query parameters")]
    InvalidQueryParams(#[from] QueryRejection),
    #[error("Invalid CSRF token")]
    InvalidCsrfToken { csrf_token: CsrfToken },
    #[error("Exchange code already received")]
    DuplicateExchangeCodeRequest,
}

impl IntoResponse for HandlerError {
    fn into_response(self) -> axum::response::Response {
        match self {
            HandlerError::InvalidQueryParams(_)
            | Self::InvalidCsrfToken { .. }
            | Self::DuplicateExchangeCodeRequest => {
                (StatusCode::BAD_REQUEST, self.to_string()).into_response()
            }
        }
    }
}

#[derive(Clone, Debug, Builder)]
pub struct RedirectHandler {
    csrf_token: CsrfToken,
    sender: OneshotSender<String>,
}

impl<T> Service<Request<T>> for RedirectHandler
where
    T: Send + 'static,
{
    type Response = Response;
    type Error = Infallible;
    type Future = ResponseFuture<Result<Self::Response, Self::Error>>;

    fn poll_ready(
        &mut self,
        _: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        std::task::Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request<T>) -> Self::Future {
        let csrf_token_secret = self.csrf_token.secret().to_string();
        let mut sender = self.sender.clone();
        let fut = async move {
            let query_params = Query::<QueryParams>::try_from_uri(req.uri());
            match query_params {
                Ok(query_params) => {
                    let csrf_token = query_params.state.clone();
                    if csrf_token.secret() != &csrf_token_secret {
                        Ok(HandlerError::InvalidCsrfToken { csrf_token }.into_response())
                    } else {
                        let send_result = sender.send(query_params.code.to_string()).await;
                        match send_result {
                            Ok(_) => Ok((
                                StatusCode::OK,
                                "Authentication successful. You may close this window.",
                            )
                                .into_response()),
                            Err(_) => {
                                tracing::error!("Already sent authorization code to receiver.");
                                Ok(HandlerError::DuplicateExchangeCodeRequest.into_response())
                            }
                        }
                    }
                }
                Err(err) => Ok(HandlerError::InvalidQueryParams(err).into_response()),
            }
        };
        Box::pin(fut)
    }
}

#[cfg(test)]
mod tests {
    use axum::extract::Request;
    use http::{Method, StatusCode, Uri};
    use oauth2::CsrfToken;
    use rover_http::body::body_to_bytes;
    use rstest::{fixture, rstest};
    use speculoos::prelude::*;
    use tokio::sync::oneshot;
    use tower::Service;

    use crate::oauth2::authorization_flow::redirect::handler::RedirectHandler;

    #[fixture]
    fn csrf_token() -> CsrfToken {
        CsrfToken::new_random()
    }

    #[fixture]
    fn exchange_code() -> String {
        "exchange_code".to_string()
    }

    #[rstest]
    #[tokio::test]
    async fn test_redirect_handler_success(exchange_code: String, csrf_token: CsrfToken) {
        let request = Request::builder()
            .method(Method::GET)
            .uri(
                Uri::builder()
                    .path_and_query(format!(
                        "/?code={}&state={}",
                        exchange_code,
                        csrf_token.secret()
                    ))
                    .build()
                    .unwrap(),
            )
            .body("".to_string())
            .unwrap();
        let (sender, receiver) = oneshot::channel();
        let mut redirect_handler = RedirectHandler::builder()
            .csrf_token(csrf_token)
            .sender(sender.into())
            .build();
        let result = redirect_handler.call(request).await;
        assert_that!(result).is_ok();
        let mut resp = result.unwrap();
        assert_that!(resp.status()).is_equal_to(StatusCode::OK);
        let body = body_to_bytes(resp.body_mut()).await.unwrap();
        let body = str::from_utf8(&body).unwrap();
        assert_that!(body).contains("Authentication successful");
        let receiver_result = receiver.await;
        assert_that!(receiver_result)
            .is_ok()
            .is_equal_to(exchange_code);
    }

    #[rstest]
    #[tokio::test]
    async fn test_redirect_handler_sender_error(exchange_code: String, csrf_token: CsrfToken) {
        let request = Request::builder()
            .method(Method::GET)
            .uri(
                Uri::builder()
                    .path_and_query(format!(
                        "/?code={}&state={}",
                        exchange_code,
                        csrf_token.secret()
                    ))
                    .build()
                    .unwrap(),
            )
            .body("".to_string())
            .unwrap();
        let (sender, receiver) = oneshot::channel();
        let mut redirect_handler = RedirectHandler::builder()
            .csrf_token(csrf_token)
            .sender(sender.into())
            .build();
        let result = redirect_handler.call(request.clone()).await;
        assert_that!(result).is_ok();
        let mut resp = result.unwrap();
        assert_that!(resp.status()).is_equal_to(StatusCode::OK);
        let body = body_to_bytes(resp.body_mut()).await.unwrap();
        let body = str::from_utf8(&body).unwrap();
        assert_that!(body).contains("Authentication successful");
        let receiver_result = receiver.await;
        assert_that!(receiver_result)
            .is_ok()
            .is_equal_to(exchange_code);

        let result = redirect_handler.call(request).await;
        assert_that!(result).is_ok();
        let mut resp = result.unwrap();
        assert_that!(resp.status()).is_equal_to(StatusCode::BAD_REQUEST);
        let body = body_to_bytes(resp.body_mut()).await.unwrap();
        let body = str::from_utf8(&body).unwrap();
        assert_that!(body).is_equal_to("Exchange code already received");
    }

    #[rstest]
    #[tokio::test]
    async fn test_redirect_handler_invalid_csrf_token(
        exchange_code: String,
        csrf_token: CsrfToken,
    ) {
        let request = Request::builder()
            .method(Method::GET)
            .uri(
                Uri::builder()
                    .path_and_query(format!(
                        "/?code={}&state={}",
                        exchange_code, "nottherightcode"
                    ))
                    .build()
                    .unwrap(),
            )
            .body("".to_string())
            .unwrap();
        let (sender, receiver) = oneshot::channel();
        let mut redirect_handler = RedirectHandler::builder()
            .csrf_token(csrf_token)
            .sender(sender.into())
            .build();
        let result = redirect_handler.call(request.clone()).await;
        assert_that!(result).is_ok();
        let mut resp = result.unwrap();
        assert_that!(resp.status()).is_equal_to(StatusCode::BAD_REQUEST);
        let body = body_to_bytes(resp.body_mut()).await.unwrap();
        let body = str::from_utf8(&body).unwrap();
        assert_that!(body).contains("Invalid CSRF token");
        assert_that!(receiver.is_empty()).is_true();
    }

    #[rstest]
    #[case::missing_state(format!("/?code={}", exchange_code()))]
    #[case::missing_code(format!("/?state={}", csrf_token.secret()))]
    #[case::missing_both("/".to_string())]
    #[tokio::test]
    async fn test_redirect_handler_invalid_request(
        csrf_token: CsrfToken,
        #[case] path_and_query: String,
    ) {
        let request = Request::builder()
            .method(Method::GET)
            .uri(
                Uri::builder()
                    .path_and_query(path_and_query)
                    .build()
                    .unwrap(),
            )
            .body("".to_string())
            .unwrap();
        let (sender, receiver) = oneshot::channel();
        let mut redirect_handler = RedirectHandler::builder()
            .csrf_token(csrf_token)
            .sender(sender.into())
            .build();
        let result = redirect_handler.call(request.clone()).await;
        assert_that!(result).is_ok();
        let mut resp = result.unwrap();
        assert_that!(resp.status()).is_equal_to(StatusCode::BAD_REQUEST);
        let body = body_to_bytes(resp.body_mut()).await.unwrap();
        let body = str::from_utf8(&body).unwrap();
        assert_that!(body).contains("Invalid query parameters");
        assert_that!(receiver.is_empty()).is_true();
    }
}

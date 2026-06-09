use std::time::Duration;

use bytes::Bytes;
use httpmock::prelude::*;
use rover_auth::oauth2::{
    AccessToken, RefreshToken as OauthRefreshToken,
    refresh_token::{RefreshToken as RefreshTokenSvc, RefreshTokenRequest},
    revoke_token::{RevokeToken, RevokeTokenError, RevokeTokenRequest},
};
use rover_http::{Full, ReqwestService};
use rstest::rstest;
use speculoos::prelude::*;
use tower::ServiceExt;
use url::Url;

#[rstest]
#[tokio::test]
#[timeout(Duration::from_secs(5))]
async fn test_refresh_token_exchanges_for_new_tokens() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/token");
        then.status(200)
            .header("content-type", "application/json")
            .body(r#"{"access_token":"new_access","token_type":"Bearer","refresh_token":"new_refresh"}"#);
    });

    let token_url = Url::parse(&format!("http://{}/token", server.address())).unwrap();

    let req = RefreshTokenRequest::builder()
        .client_id("test-client".to_string())
        .token_url(token_url)
        .refresh_token("old_refresh_token".to_string())
        .build();

    let http_service = ReqwestService::builder()
        .client(reqwest::Client::default())
        .build()
        .unwrap();

    let svc: RefreshTokenSvc<ReqwestService, Full<Bytes>> = RefreshTokenSvc::new(http_service);
    let result = svc.oneshot(req).await;
    let resp = assert_that!(result).is_ok().subject;

    assert_that!(resp.access_token.secret()).is_equal_to(&"new_access".to_string());
    assert_that!(resp.refresh_token.as_ref().unwrap().secret())
        .is_equal_to(&"new_refresh".to_string());
}

// The oauth2 crate (RFC 7009) requires HTTPS for the revocation endpoint as a
// security measure. These tests verify that the service correctly enforces this
// and returns the right error variant for insecure URLs.

#[rstest]
#[tokio::test]
#[timeout(Duration::from_secs(5))]
async fn test_revoke_access_token_requires_https() {
    let token = AccessToken::new("my_access_token".to_string());
    let req = RevokeTokenRequest::builder()
        .client_id("test-client".to_string())
        .revocation_url(Url::parse("http://example.com/revoke").unwrap())
        .token(token)
        .build();

    let http_service = ReqwestService::builder()
        .client(reqwest::Client::default())
        .build()
        .unwrap();

    let svc: RevokeToken<ReqwestService, Full<Bytes>> = RevokeToken::new(http_service);
    let result = svc.oneshot(req).await;
    let err = assert_that!(result).is_err().subject;

    assert_that!(err).matches(|e| matches!(e, RevokeTokenError::OauthConfiguration(_)));
}

#[rstest]
#[tokio::test]
#[timeout(Duration::from_secs(5))]
async fn test_revoke_refresh_token_requires_https() {
    let token = OauthRefreshToken::new("my_refresh_token".to_string());
    let req = RevokeTokenRequest::builder()
        .client_id("test-client".to_string())
        .revocation_url(Url::parse("http://example.com/revoke").unwrap())
        .token(token)
        .build();

    let http_service = ReqwestService::builder()
        .client(reqwest::Client::default())
        .build()
        .unwrap();

    let svc: RevokeToken<ReqwestService, Full<Bytes>> = RevokeToken::new(http_service);
    let result = svc.oneshot(req).await;
    let err = assert_that!(result).is_err().subject;

    assert_that!(err).matches(|e| matches!(e, RevokeTokenError::OauthConfiguration(_)));
}

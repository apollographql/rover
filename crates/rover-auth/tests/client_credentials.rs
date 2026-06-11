use std::time::Duration;

use bytes::Bytes;
use httpmock::prelude::*;
use rover_auth::oauth2::{
    Scope,
    client_credentials::{ClientCredentials, ClientCredentialsError, ClientCredentialsRequest},
};
use rover_http::{Full, ReqwestService};
use rstest::rstest;
use speculoos::prelude::*;
use tower::ServiceExt;
use url::Url;

fn http_service() -> ReqwestService {
    ReqwestService::builder()
        .client(reqwest::Client::default())
        .build()
        .unwrap()
}

#[rstest]
#[tokio::test]
#[timeout(Duration::from_secs(5))]
async fn test_client_credentials_exchanges_for_access_token() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/token");
        then.status(200)
            .header("content-type", "application/json")
            .body(r#"{"access_token":"my_access_token","token_type":"Bearer"}"#);
    });

    let token_url = Url::parse(&format!("http://{}/token", server.address())).unwrap();

    let req = ClientCredentialsRequest::builder()
        .client_id("test-client".to_string())
        .client_secret("test-secret".to_string())
        .token_url(token_url)
        .scopes(vec![Scope::new("rover:cli".to_string())])
        .build()
        .unwrap();

    let svc: ClientCredentials<ReqwestService, Full<Bytes>> =
        ClientCredentials::new(http_service());
    let result = svc.oneshot(req).await;
    let resp = assert_that!(result).is_ok().subject;

    assert_that!(resp.access_token.secret()).is_equal_to(&"my_access_token".to_string());
    assert_that!(resp.expires_in).is_none();
}

#[rstest]
#[tokio::test]
#[timeout(Duration::from_secs(5))]
async fn test_client_credentials_response_includes_expires_in() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/token");
        then.status(200)
            .header("content-type", "application/json")
            .body(r#"{"access_token":"my_access_token","token_type":"Bearer","expires_in":3600}"#);
    });

    let token_url = Url::parse(&format!("http://{}/token", server.address())).unwrap();

    let req = ClientCredentialsRequest::builder()
        .client_id("test-client".to_string())
        .client_secret("test-secret".to_string())
        .token_url(token_url)
        .scopes(vec![])
        .build()
        .unwrap();

    let svc: ClientCredentials<ReqwestService, Full<Bytes>> =
        ClientCredentials::new(http_service());
    let result = svc.oneshot(req).await;
    let resp = assert_that!(result).is_ok().subject;

    assert_that!(resp.expires_in).is_equal_to(Some(Duration::from_secs(3600)));
}

#[rstest]
#[tokio::test]
#[timeout(Duration::from_secs(5))]
async fn test_client_credentials_server_oauth_error() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/token");
        then.status(400)
            .header("content-type", "application/json")
            .body(
                r#"{"error":"invalid_client","error_description":"Client authentication failed"}"#,
            );
    });

    let token_url = Url::parse(&format!("http://{}/token", server.address())).unwrap();

    let req = ClientCredentialsRequest::builder()
        .client_id("bad-client".to_string())
        .client_secret("bad-secret".to_string())
        .token_url(token_url)
        .scopes(vec![])
        .build()
        .unwrap();

    let svc: ClientCredentials<ReqwestService, Full<Bytes>> =
        ClientCredentials::new(http_service());
    let result = svc.oneshot(req).await;

    assert_that!(result)
        .is_err()
        .matches(|e| matches!(e, ClientCredentialsError::OAuth(_)));
}

#[rstest]
#[tokio::test]
#[timeout(Duration::from_secs(5))]
async fn test_client_credentials_server_parse_error() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/token");
        then.status(200)
            .header("content-type", "application/json")
            .body(r#"{"unexpected":"value"}"#);
    });

    let token_url = Url::parse(&format!("http://{}/token", server.address())).unwrap();

    let req = ClientCredentialsRequest::builder()
        .client_id("test-client".to_string())
        .client_secret("test-secret".to_string())
        .token_url(token_url)
        .scopes(vec![])
        .build()
        .unwrap();

    let svc: ClientCredentials<ReqwestService, Full<Bytes>> =
        ClientCredentials::new(http_service());
    let result = svc.oneshot(req).await;

    assert_that!(result)
        .is_err()
        .matches(|e| matches!(e, ClientCredentialsError::Parse { .. }));
}

use std::time::Duration;

use httpmock::prelude::*;
use rover_auth::oauth2::register::{Register, RegisterError, RegisterRequest};
use rover_http::ReqwestService;
use rstest::rstest;
use speculoos::prelude::*;
use tower::ServiceExt;
use url::Url;

#[rstest]
#[tokio::test]
#[timeout(Duration::from_secs(5))]
async fn test_client_registration_returns_client_id() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST)
            .path("/register")
            .header("content-type", "application/json");
        then.status(200)
            .header("content-type", "application/json")
            .body(r#"{"client_id":"test-client-id"}"#);
    });

    let register_url = Url::parse(&format!("http://{}/register", server.address())).unwrap();
    let redirect_url = Url::parse("http://127.0.0.1:0/callback").unwrap();

    let req = RegisterRequest::builder()
        .register_url(register_url)
        .redirect_url(redirect_url)
        .build();

    let http_service = ReqwestService::builder()
        .client(reqwest::Client::default())
        .build()
        .unwrap();

    let result = Register::new(http_service).oneshot(req).await;
    let resp = assert_that!(result).is_ok().subject;
    assert_that!(resp.client_id.as_str()).is_equal_to("test-client-id");
}

#[rstest]
#[tokio::test]
#[timeout(Duration::from_secs(5))]
async fn test_client_registration_sends_expected_scopes() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(POST)
            .path("/register")
            .body_includes("rover:cli")
            .body_includes("openid");
        then.status(200)
            .header("content-type", "application/json")
            .body(r#"{"client_id":"scoped-client"}"#);
    });

    let register_url = Url::parse(&format!("http://{}/register", server.address())).unwrap();
    let redirect_url = Url::parse("http://127.0.0.1:8080/callback").unwrap();

    let req = RegisterRequest::builder()
        .register_url(register_url)
        .redirect_url(redirect_url)
        .build();

    let http_service = ReqwestService::builder()
        .client(reqwest::Client::default())
        .build()
        .unwrap();

    let result = Register::new(http_service).oneshot(req).await;
    assert_that!(result).is_ok();
    mock.assert();
}

// Without this, the device-authorization endpoint rejects `rover auth login
// --no-browser` with `unsupported_grant_type`, since the registered client
// never claimed the device-code grant in the first place.
#[rstest]
#[tokio::test]
#[timeout(Duration::from_secs(5))]
async fn test_client_registration_sends_the_device_code_grant_type() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(POST)
            .path("/register")
            .body_includes("authorization_code")
            .body_includes("urn:ietf:params:oauth:grant-type:device_code");
        then.status(200)
            .header("content-type", "application/json")
            .body(r#"{"client_id":"device-code-client"}"#);
    });

    let register_url = Url::parse(&format!("http://{}/register", server.address())).unwrap();
    let redirect_url = Url::parse("http://127.0.0.1:8080/callback").unwrap();

    let req = RegisterRequest::builder()
        .register_url(register_url)
        .redirect_url(redirect_url)
        .build();

    let http_service = ReqwestService::builder()
        .client(reqwest::Client::default())
        .build()
        .unwrap();

    let result = Register::new(http_service).oneshot(req).await;
    assert_that!(result).is_ok();
    mock.assert();
}

#[rstest]
#[tokio::test]
#[timeout(Duration::from_secs(5))]
async fn test_client_registration_server_error() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/register");
        then.status(500);
    });

    let req = RegisterRequest::builder()
        .register_url(Url::parse(&format!("http://{}/register", server.address())).unwrap())
        .redirect_url(Url::parse("http://127.0.0.1:0/callback").unwrap())
        .build();

    let http_service = ReqwestService::builder()
        .client(reqwest::Client::default())
        .build()
        .unwrap();

    let result = Register::new(http_service).oneshot(req).await;
    assert_that!(result)
        .is_err()
        .matches(|e| matches!(e, RegisterError::Http(_)));
}

#[rstest]
#[tokio::test]
#[timeout(Duration::from_secs(5))]
async fn test_client_registration_invalid_json_response() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/register");
        then.status(200)
            .header("content-type", "application/json")
            .body("not valid json");
    });

    let req = RegisterRequest::builder()
        .register_url(Url::parse(&format!("http://{}/register", server.address())).unwrap())
        .redirect_url(Url::parse("http://127.0.0.1:0/callback").unwrap())
        .build();

    let http_service = ReqwestService::builder()
        .client(reqwest::Client::default())
        .build()
        .unwrap();

    let result = Register::new(http_service).oneshot(req).await;
    assert_that!(result)
        .is_err()
        .matches(|e| matches!(e, RegisterError::Deserialize(_)));
}

use std::time::Duration;

use httpmock::prelude::*;
use rover_auth::oauth2::{
    Scope,
    authorization_flow::{AuthorizationFlow, redirect::server::AxumRedirectServer},
};
use rover_http::{Full, ReqwestService};
use rover_open::MockOpenUrl;
use rover_print::print::MockPrint;
use rstest::rstest;
use speculoos::prelude::*;
use url::Url;

#[rstest]
#[tokio::test]
#[timeout(Duration::from_secs(10))]
async fn test_full_pkce_flow_authorize_and_exchange() {
    let token_server = MockServer::start();
    token_server.mock(|when, then| {
        when.method(POST).path("/token");
        then.status(200)
            .header("content-type", "application/json")
            .body(r#"{"access_token":"test_access_token","token_type":"Bearer","refresh_token":"test_refresh_token"}"#);
    });

    let token_url = Url::parse(&format!("http://{}/token", token_server.address())).unwrap();

    let flow = AuthorizationFlow::builder()
        .client_id("test-client-id".to_string())
        .authorization_url(Url::parse("https://example.com/authorize").unwrap())
        .token_url(token_url)
        .build();

    let mut mock_open_url = MockOpenUrl::new();
    mock_open_url.expect_open_url().times(1).returning(|url| {
        let state = url
            .query_pairs()
            .find(|(k, _)| k == "state")
            .map(|(_, v)| v.into_owned())
            .expect("state param missing from auth URL");
        let redirect_uri = url
            .query_pairs()
            .find(|(k, _)| k == "redirect_uri")
            .map(|(_, v)| v.into_owned())
            .expect("redirect_uri param missing from auth URL");
        let port = Url::parse(&redirect_uri)
            .expect("invalid redirect_uri")
            .port()
            .expect("no port in redirect_uri");

        tokio::spawn(async move {
            let callback = format!("http://127.0.0.1:{}/?code=authcode&state={}", port, state);
            reqwest::get(&callback)
                .await
                .expect("OAuth callback request failed");
        });
        Ok(())
    });

    let mock_print = MockPrint::new();

    let authorize_result = flow
        .authorize(
            vec![Scope::new("rover:cli".to_string())],
            &mock_open_url,
            &mock_print,
            AxumRedirectServer::default(),
        )
        .await;
    assert_that!(authorize_result).is_ok();
    let with_code = authorize_result.unwrap();

    let http_service = ReqwestService::builder()
        .client(reqwest::Client::default())
        .build()
        .unwrap();

    let exchange_result = with_code
        .exchange_code::<ReqwestService, Full<bytes::Bytes>>(http_service)
        .await;
    let response = assert_that!(exchange_result).is_ok().subject;

    assert_that!(response.access_token.secret()).is_equal_to(&"test_access_token".to_string());
    assert_that!(response.refresh_token.as_ref().unwrap().secret())
        .is_equal_to(&"test_refresh_token".to_string());
}

#[rstest]
#[tokio::test]
#[timeout(Duration::from_secs(10))]
async fn test_authorize_open_url_failure_falls_back_to_printed_url() {
    let token_server = MockServer::start();
    token_server.mock(|when, then| {
        when.method(POST).path("/token");
        then.status(200)
            .header("content-type", "application/json")
            .body(r#"{"access_token":"fallback_token","token_type":"Bearer"}"#);
    });

    let token_url = Url::parse(&format!("http://{}/token", token_server.address())).unwrap();

    let flow = AuthorizationFlow::builder()
        .client_id("test-client-id".to_string())
        .authorization_url(Url::parse("https://example.com/authorize").unwrap())
        .token_url(token_url)
        .build();

    let mut mock_open_url = MockOpenUrl::new();
    mock_open_url.expect_open_url().times(1).returning(|url| {
        // Simulate browser open failure; still spawn the callback so the flow can complete.
        let state = url
            .query_pairs()
            .find(|(k, _)| k == "state")
            .map(|(_, v)| v.into_owned())
            .unwrap();
        let redirect_uri = url
            .query_pairs()
            .find(|(k, _)| k == "redirect_uri")
            .map(|(_, v)| v.into_owned())
            .unwrap();
        let port = Url::parse(&redirect_uri).unwrap().port().unwrap();
        tokio::spawn(async move {
            let callback = format!(
                "http://127.0.0.1:{}/?code=fallbackcode&state={}",
                port, state
            );
            reqwest::get(&callback).await.unwrap();
        });
        Err(std::io::Error::other("browser unavailable"))
    });

    let mut mock_print = MockPrint::new();
    mock_print.expect_print().times(1).returning(|_| Ok(()));

    let authorize_result = flow
        .authorize(
            vec![Scope::new("rover:cli".to_string())],
            &mock_open_url,
            &mock_print,
            AxumRedirectServer::default(),
        )
        .await;
    assert_that!(authorize_result).is_ok();
    let with_code = authorize_result.unwrap();

    let http_service = ReqwestService::builder()
        .client(reqwest::Client::default())
        .build()
        .unwrap();

    let exchange_result = with_code
        .exchange_code::<ReqwestService, Full<bytes::Bytes>>(http_service)
        .await;
    let response = assert_that!(exchange_result).is_ok().subject;

    assert_that!(response.access_token.secret()).is_equal_to(&"fallback_token".to_string());
}

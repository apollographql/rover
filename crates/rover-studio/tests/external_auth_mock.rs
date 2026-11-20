#![cfg(feature = "testing")]

use futures::{future::ready, FutureExt};
use rover_auth::external_token::ExternalAccessToken;
use rover_studio::service::external_auth::mock::{MockTokenService, TokenRequest};
use rstest::rstest;
use speculoos::prelude::*;
use tower::ServiceExt;

#[rstest]
fn external_auth_mock_is_available_to_consumers() {
    let token = ExternalAccessToken::parse(b"mock-access-token").unwrap();
    let response = token.clone();
    let mut source = MockTokenService::new();
    rover_tower::expect_poll_ready!(source);
    source
        .expect_call()
        .once()
        .return_once(|_: TokenRequest| ready(Ok(response)));

    assert_that!(source.oneshot(()).now_or_never())
        .is_some()
        .is_ok()
        .is_equal_to(token);
}

#![cfg(feature = "testing")]

use futures::{FutureExt, future::ready};
use rover_auth::external_token::{
    ExternalAccessToken,
    mock::{MockTokenSourceService, TokenSourceRequest},
};
use rstest::rstest;
use speculoos::prelude::*;
use tower::ServiceExt;

#[rstest]
fn token_source_mock_is_available_to_consumers() {
    let token = ExternalAccessToken::parse(b"mock-access-token").unwrap();
    let response = token.clone();
    let mut source = MockTokenSourceService::new();
    rover_tower::expect_poll_ready!(source);
    source
        .expect_call()
        .once()
        .return_once(|_: TokenSourceRequest| ready(Ok(response)));

    assert_that!(source.oneshot(()).now_or_never().unwrap()).is_equal_to(Ok(token));
}

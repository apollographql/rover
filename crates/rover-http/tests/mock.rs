#![cfg(feature = "test")]

use bytes::Bytes;
use futures::{future::ready, FutureExt};
use http::StatusCode;
use rover_http::{mock::MockHttpService, BodyExt, Full, HttpRequest, HttpResponse};
use rstest::rstest;
use speculoos::prelude::*;
use tower::ServiceExt;

#[rstest]
fn http_mock_is_available_to_consumers() {
    let mut service: rover_http::test::MockHttpService = MockHttpService::new();
    rover_tower::expect_poll_ready!(service);
    service.expect_call().once().return_once(|_| {
        ready(Ok(HttpResponse::new(Full::new(Bytes::from_static(
            b"mock-response",
        )))))
    });

    let response = service
        .oneshot(HttpRequest::default())
        .now_or_never()
        .unwrap()
        .map(|response| {
            (
                response.status(),
                response
                    .into_body()
                    .collect()
                    .now_or_never()
                    .unwrap()
                    .unwrap()
                    .to_bytes(),
            )
        })
        .map_err(|error| error.to_string());

    assert_that!(response).is_equal_to(Ok((StatusCode::OK, Bytes::from_static(b"mock-response"))));
}

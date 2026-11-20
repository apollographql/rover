#![cfg(feature = "test")]

use futures::{future::ready, FutureExt};
use rover_tower::mock::MockCloneService;
use rstest::rstest;
use speculoos::prelude::*;
use tower::ServiceExt;

mod mock {
    pub type EchoRequest = String;
    pub type EchoError = std::convert::Infallible;

    rover_tower::mock_service!(Echo, EchoRequest, String, EchoError);
}

#[rstest]
fn mock_helpers_are_available_to_consumers() {
    let mut service = mock::MockEchoService::new();
    rover_tower::mock::expect_poll_ready!(service);
    service
        .expect_call()
        .once()
        .return_once(|request| ready(Ok(request)));
    let service: rover_tower::test::MockCloneService<_> = MockCloneService::new(service);

    assert_that!(service.oneshot("mock-request".to_string()).now_or_never())
        .is_some()
        .is_ok()
        .is_equal_to("mock-request".to_string());
}

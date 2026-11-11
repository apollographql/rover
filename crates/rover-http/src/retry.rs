//! Middleware for retrying HTTP requests

use std::{
    cell::OnceCell,
    time::{Duration, Instant},
};

use http::StatusCode;
use tap::TapFallible;
use tower::{
    layer::util::Stack,
    retry::{
        backoff::{Backoff, ExponentialBackoff, ExponentialBackoffMaker, MakeBackoff},
        Policy, RetryLayer,
    },
    util::rng::HasherRng,
};

use super::HttpServiceError;
use crate::{timeout::TimeoutLayer, HttpRequest, HttpResponse};

/// Retries a request for up to `retry_budget`, bounding each attempt by `attempt_timeout`.
///
/// The timeout sits inside the retry, so a hung attempt times out and is retried rather than
/// consuming the whole budget. Pass the result to [`tower::ServiceBuilder::layer`]; the inner
/// service must be `Clone`, since every attempt is a fresh call.
pub fn retry_with_attempt_timeout(
    retry_budget: Duration,
    attempt_timeout: Duration,
) -> Stack<TimeoutLayer, RetryLayer<RetryPolicy>> {
    Stack::new(
        TimeoutLayer::new(attempt_timeout),
        RetryLayer::new(RetryPolicy::new(retry_budget)),
    )
}

/// [`Policy`] implementation that describes whetheer to retry a request
#[derive(Clone, Debug)]
pub struct RetryPolicy {
    start_time: OnceCell<Instant>,
    max_elapsed_time: Duration,
    backoff: ExponentialBackoff,
}

impl RetryPolicy {
    /// Constructs a new [`RetryPolicy`]
    pub fn new(max_elapsed_time: Duration) -> RetryPolicy {
        let backoff = ExponentialBackoffMaker::new(
            Duration::from_millis(500),
            Duration::from_millis(60000),
            0.99,
            HasherRng::default(),
        )
        .tap_err(|err| tracing::error!("{:?}", err))
        .unwrap()
        .make_backoff();
        RetryPolicy {
            start_time: OnceCell::new(),
            max_elapsed_time,
            backoff,
        }
    }

    /// Dictates whether a request can be retried, based on an optional maximum elapsed time
    pub fn can_retry(&self) -> bool {
        self.start_time.get_or_init(Instant::now).elapsed() < self.max_elapsed_time
    }
}

/// Whether a response with the given status code is worth retrying.
///
/// The legacy blocking client in `rover-client` defers to this too, so the two request paths
/// can't drift apart on which failures are worth a second attempt.
pub fn is_retryable_status(status: StatusCode) -> bool {
    status.is_server_error()
        || matches!(
            status,
            StatusCode::REQUEST_TIMEOUT | StatusCode::TOO_EARLY | StatusCode::TOO_MANY_REQUESTS
        )
}

/// How many earlier attempts of the same logical request came before this one.
///
/// A request with no `RetryCount` extension is on its first attempt, so it has retried zero times.
/// [`RetryPolicy`] raises the count each time it retries, so the attempt about to be sent knows
/// how many came before it. The count lives on the request, not in the policy, so each new
/// request starts again from zero without anything resetting it.
///
/// It counts every attempt of the logical request, across all the [`RetryPolicy`]s that handle it.
/// A service that retries inside another one that retries would count both, as one running total.
/// A retry [`Policy`] of any other kind has to raise this count in its own `retry`, as
/// [`RetryPolicy`] does, for the retry count sent with each attempt to be right.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RetryCount(pub u32);

impl RetryCount {
    /// The retry count of `request`, which is zero if nothing has retried it
    pub fn of<B>(request: &http::Request<B>) -> RetryCount {
        request
            .extensions()
            .get::<RetryCount>()
            .copied()
            .unwrap_or_default()
    }
}

impl RetryPolicy {
    /// How long to wait before retrying, or `None` if `result` shouldn't be retried
    fn next_delay(
        &mut self,
        result: &Result<HttpResponse, HttpServiceError>,
    ) -> Option<tokio::time::Sleep> {
        if !self.can_retry() {
            return None;
        }
        let retryable = match result {
            Err(HttpServiceError::TimedOut)
            | Err(HttpServiceError::Connect(_))
            | Err(HttpServiceError::Body(_))
            | Err(HttpServiceError::Decode(_))
            | Err(HttpServiceError::Request(_))
            | Err(HttpServiceError::Closed(_)) => true,
            Err(_) => false,
            Ok(resp) => is_retryable_status(resp.status()),
        };
        retryable.then(|| self.backoff.next_backoff())
    }
}

impl Policy<HttpRequest, HttpResponse, HttpServiceError> for RetryPolicy {
    type Future = tokio::time::Sleep;
    fn retry(
        &mut self,
        req: &mut HttpRequest,
        result: &mut Result<HttpResponse, HttpServiceError>,
    ) -> Option<Self::Future> {
        let delay = self.next_delay(result)?;
        // `req` is the request that will be sent next, so it carries one more retry than the
        // attempt that just failed.
        let RetryCount(retries) = RetryCount::of(req);
        req.extensions_mut()
            .insert(RetryCount(retries.saturating_add(1)));
        Some(delay)
    }

    fn clone_request(&mut self, req: &HttpRequest) -> Option<HttpRequest> {
        Some(req.clone())
    }
}

#[cfg(test)]
mod tests {

    use std::time::Duration;

    use anyhow::Result;
    use http::StatusCode;
    use http_body_util::Full;
    use httpmock::MockServer;
    use rstest::{fixture, rstest};
    use speculoos::prelude::*;
    use tower::{Service, ServiceBuilder, ServiceExt};

    use super::{retry_with_attempt_timeout, RetryPolicy};
    use crate::{HttpService, HttpServiceError, ReqwestService};

    #[fixture]
    pub fn raw_service() -> HttpService {
        let client = reqwest::Client::default();
        ReqwestService::builder()
            .client(client)
            .build()
            .unwrap()
            .boxed_clone()
    }

    #[fixture]
    pub fn retry_policy() -> RetryPolicy {
        RetryPolicy::new(Duration::from_millis(1500))
    }

    #[fixture]
    pub fn retry_service(retry_policy: RetryPolicy, raw_service: HttpService) -> HttpService {
        ServiceBuilder::new()
            .retry(retry_policy)
            .service(raw_service)
            .boxed_clone()
    }

    #[rstest]
    #[tokio::test]
    pub async fn test_backoff(mut retry_service: HttpService) -> Result<()> {
        let server = MockServer::start();
        let addr = server.address().to_string();
        println!("addr: {addr}");
        let uri = format!("http://{}/", addr);

        let mock = server.mock(|when, then| {
            when.method(httpmock::Method::GET).path("/");
            then.status(500).body("");
        });

        let request = http::Request::builder()
            .uri(uri)
            .method(http::Method::GET)
            .body(Full::default())?;

        let resp = retry_service.call(request).await;

        mock.assert_calls(3);

        assert_that!(resp)
            .is_ok()
            .matches(|resp| resp.status() == StatusCode::INTERNAL_SERVER_ERROR);
        Ok(())
    }

    #[rstest]
    #[case::unauthorized(StatusCode::UNAUTHORIZED)]
    #[case::forbidden(StatusCode::FORBIDDEN)]
    #[case::not_found(StatusCode::NOT_FOUND)]
    #[case::bad_request(StatusCode::BAD_REQUEST)]
    #[tokio::test]
    pub async fn non_retryable_4xx_is_not_retried(
        #[case] status: StatusCode,
        mut retry_service: HttpService,
    ) -> Result<()> {
        let server = MockServer::start();
        let uri = format!("http://{}/", server.address());

        let mock = server.mock(|when, then| {
            when.method(httpmock::Method::GET).path("/");
            then.status(status.as_u16()).body("");
        });

        let request = http::Request::builder()
            .uri(uri)
            .method(http::Method::GET)
            .body(Full::default())?;

        let resp = retry_service.call(request).await;

        mock.assert_calls(1);
        assert_that!(resp)
            .is_ok()
            .matches(|resp| resp.status() == status);
        Ok(())
    }

    #[rstest]
    #[case::request_timeout(StatusCode::REQUEST_TIMEOUT)]
    #[case::too_many_requests(StatusCode::TOO_MANY_REQUESTS)]
    #[tokio::test]
    pub async fn retryable_4xx_is_retried(
        #[case] status: StatusCode,
        mut retry_service: HttpService,
    ) -> Result<()> {
        let server = MockServer::start();
        let uri = format!("http://{}/", server.address());

        let mock = server.mock(|when, then| {
            when.method(httpmock::Method::GET).path("/");
            then.status(status.as_u16()).body("");
        });

        let request = http::Request::builder()
            .uri(uri)
            .method(http::Method::GET)
            .body(Full::default())?;

        let resp = retry_service.call(request).await;

        // 1.5s budget with 500ms initial backoff → at least a couple attempts.
        assert_that!(mock.calls()).is_greater_than(1);
        assert_that!(resp)
            .is_ok()
            .matches(|resp| resp.status() == status);
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    pub async fn hung_attempt_times_out_and_is_retried(raw_service: HttpService) -> Result<()> {
        let server = MockServer::start();
        let uri = format!("http://{}/", server.address());

        // Every attempt hangs well past the per-attempt timeout.
        let mock = server.mock(|when, then| {
            when.method(httpmock::Method::GET).path("/");
            then.status(200).delay(Duration::from_secs(5));
        });

        let mut service = ServiceBuilder::new()
            .layer(retry_with_attempt_timeout(
                Duration::from_millis(1500),
                Duration::from_millis(200),
            ))
            .service(raw_service);

        let request = http::Request::builder()
            .uri(uri)
            .method(http::Method::GET)
            .body(Full::default())?;

        let resp = service.ready().await?.call(request).await;

        // Without the inner timeout, the first attempt would hang for the full 5s delay and
        // the budget would allow no second one.
        assert_that!(mock.calls()).is_greater_than(1);
        assert_that!(matches!(resp, Err(HttpServiceError::TimedOut))).is_true();
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    pub async fn prompt_success_is_attempted_once(raw_service: HttpService) -> Result<()> {
        let server = MockServer::start();
        let uri = format!("http://{}/", server.address());

        let mock = server.mock(|when, then| {
            when.method(httpmock::Method::GET).path("/");
            then.status(200).body("ok");
        });

        let mut service = ServiceBuilder::new()
            .layer(retry_with_attempt_timeout(
                Duration::from_millis(1500),
                Duration::from_millis(200),
            ))
            .service(raw_service);

        let request = http::Request::builder()
            .uri(uri)
            .method(http::Method::GET)
            .body(Full::default())?;

        let resp = service.ready().await?.call(request).await;

        mock.assert_calls(1);
        assert_that!(resp)
            .is_ok()
            .matches(|resp| resp.status() == StatusCode::OK);
        Ok(())
    }
}

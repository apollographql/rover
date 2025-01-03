//! Middleware for retrying HTTP requests

use std::{
    cell::OnceCell,
    time::{Duration, Instant},
};

use http::StatusCode;
use tap::TapFallible;
use tower::{
    retry::{
        backoff::{Backoff, ExponentialBackoff, ExponentialBackoffMaker, MakeBackoff},
        Policy,
    },
    util::rng::HasherRng,
};

use crate::{HttpRequest, HttpResponse};

use super::HttpServiceError;

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

impl Policy<HttpRequest, HttpResponse, HttpServiceError> for RetryPolicy {
    type Future = tokio::time::Sleep;
    fn retry(
        &mut self,
        _: &mut HttpRequest,
        result: &mut Result<HttpResponse, HttpServiceError>,
    ) -> Option<Self::Future> {
        if self.can_retry() {
            match result {
                Err(HttpServiceError::TimedOut(_))
                | Err(HttpServiceError::Connect(_))
                | Err(HttpServiceError::Body(_))
                | Err(HttpServiceError::Decode(_))
                | Err(HttpServiceError::Closed(_)) => Some(self.backoff.next_backoff()),
                Err(_) => None,
                Ok(resp) => {
                    let status = resp.status();
                    if status.is_client_error()
                        || status.is_server_error()
                        || status.is_redirection()
                    {
                        if matches!(status, StatusCode::BAD_REQUEST) {
                            None
                        } else {
                            Some(self.backoff.next_backoff())
                        }
                    } else {
                        None
                    }
                }
            }
        } else {
            None
        }
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

    use crate::{HttpService, ReqwestService};

    use super::RetryPolicy;

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
        RetryPolicy::new(Duration::from_millis(250))
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
        let uri = format!("http://{}/", addr);

        let mock = server.mock(|when, then| {
            when.method(httpmock::Method::GET).path("/");
            then.status(500).body("");
        });

        println!("uri: {uri}");

        let request = http::Request::builder()
            .uri(uri)
            .method(http::Method::GET)
            .body(Full::default())?;

        let resp = retry_service.call(request).await;

        mock.assert_hits(3);

        assert_that!(resp)
            .is_ok()
            .matches(|resp| resp.status() == StatusCode::INTERNAL_SERVER_ERROR);
        Ok(())
    }
}

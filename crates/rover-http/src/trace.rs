//! Adds W3C Trace Context `traceparent` and `tracestate` headers to HTTP requests
//!
//! Trace IDs, span IDs and the header's wire format come from OpenTelemetry, which is the source
//! of truth for them. What is Rover's own is when they are made: one trace ID for the whole
//! process, and a new span ID, which the header calls the parent ID, for each attempt.
//! [`TraceparentLayer`] does that for [`tower`] services, so place it _inside_ any retry layer to
//! give every attempt of a request its own parent ID and the same trace ID. [`stamp`] does it for
//! a client that retries outside of one.

use std::{
    collections::HashSet,
    fmt::Display,
    future::Future,
    pin::Pin,
    sync::{Mutex, OnceLock},
    task::Context,
};

use http::{HeaderMap, HeaderName};
pub use opentelemetry::trace::{SpanContext, SpanId, TraceId};
use opentelemetry::{
    propagation::TextMapPropagator,
    trace::{TraceContextExt, TraceFlags, TraceState},
    Context as OtelContext,
};
use opentelemetry_http::HeaderInjector;
use opentelemetry_sdk::{
    propagation::TraceContextPropagator,
    trace::{IdGenerator, RandomIdGenerator},
};
use tower::{Layer, Service};

use crate::retry::RetryCount;

const TRACEPARENT: HeaderName = HeaderName::from_static("traceparent");
const TRACESTATE: HeaderName = HeaderName::from_static("tracestate");

/// Generates a random trace ID.
///
/// OpenTelemetry's generator makes it, so it is derived from nothing about the session, the
/// machine, or the person running Rover.
pub fn new_trace_id() -> TraceId {
    RandomIdGenerator::default().new_trace_id()
}

/// The trace ID for this process, generated the first time it is asked for.
///
/// Rover is a one-shot command, so a process is an invocation: every Platform API request in one
/// run shares this ID, however many clients it builds along the way.
pub fn process_trace_id() -> TraceId {
    static PROCESS: OnceLock<TraceId> = OnceLock::new();
    *PROCESS.get_or_init(new_trace_id)
}

/// Sets `traceparent` and `tracestate` in `headers` for one more request in the trace `trace_id`.
///
/// `traceparent` carries `trace_id`, a new random span ID as its parent ID, and the sampled flag.
/// `tracestate` carries one entry, `rover=retries:<retries>`, where `retries` is how many earlier
/// attempts of the same request there were: 0 for the first attempt. Call it once per attempt of a
/// request, and use the returned [`SpanContext`] to say which IDs that attempt sent.
///
/// A `traceparent` or `tracestate` the headers already had is replaced, and is never left in place
/// beside, or instead of, the ones this makes.
pub fn stamp(trace_id: TraceId, retries: u32, headers: &mut HeaderMap) -> SpanContext {
    // A run has one trace ID, so this logs it once, with the first request. Remembering which IDs
    // were logged, rather than whether any was, keeps that true for a caller that makes its own.
    static LOGGED: OnceLock<Mutex<HashSet<TraceId>>> = OnceLock::new();
    let first_with_this_id = LOGGED
        .get_or_init(Mutex::default)
        .lock()
        .is_ok_and(|mut logged| logged.insert(trace_id));
    if first_with_this_id {
        tracing::debug!(%trace_id, "Platform API requests in this run carry trace ID {trace_id}");
    }
    let trace_state =
        TraceState::from_key_value([("rover", format!("retries:{retries}"))]).unwrap_or_default();
    let span_context = SpanContext::new(
        trace_id,
        RandomIdGenerator::default().new_span_id(),
        TraceFlags::SAMPLED,
        true,
        trace_state,
    );
    let context = OtelContext::new().with_remote_span_context(span_context.clone());
    // The propagator writes only a valid span context, and then writes a `tracestate` even if it
    // is empty. So clear both first, so that nothing the request already had can survive, and
    // don't send an empty one, which W3C says to avoid.
    headers.remove(TRACEPARENT);
    headers.remove(TRACESTATE);
    TraceContextPropagator::new().inject_context(&context, &mut HeaderInjector(headers));
    if span_context.trace_state().header().is_empty() {
        headers.remove(TRACESTATE);
    }
    span_context
}

/// Logs, at debug level, that the attempt `span_context` was stamped for failed.
///
/// The entry carries the trace ID, the attempt's parent ID and its retry count, which is what a
/// person needs to give support so that attempt can be found.
pub fn log_failed_attempt(span_context: &SpanContext, retries: u32, failure: impl Display) {
    tracing::debug!(
        trace_id = %span_context.trace_id(),
        parent_id = %span_context.span_id(),
        retries,
        "Platform API request attempt failed: {failure}"
    );
}

/// Layer that applies [`TraceparentService`], which adds `traceparent` and `tracestate` headers to
/// HTTP requests
#[derive(Clone, Copy, Debug)]
pub struct TraceparentLayer {
    trace_id: TraceId,
}

impl TraceparentLayer {
    /// Constructs a new [`TraceparentLayer`] whose requests all carry `trace_id`
    pub const fn new(trace_id: TraceId) -> TraceparentLayer {
        TraceparentLayer { trace_id }
    }
}

impl<S> Layer<S> for TraceparentLayer {
    type Service = TraceparentService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        TraceparentService {
            trace_id: self.trace_id,
            inner,
        }
    }
}

/// Middleware that adds trace headers to every HTTP request it sends, as [`stamp`] does, with the
/// retry count [`RetryPolicy`](crate::retry::RetryPolicy) left on the request
#[derive(Clone, Debug)]
pub struct TraceparentService<S> {
    trace_id: TraceId,
    inner: S,
}

impl<S, B, RB> Service<http::Request<B>> for TraceparentService<S>
where
    S: Service<http::Request<B>, Response = http::Response<RB>>,
    S::Future: Send + 'static,
    S::Error: Display,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<S::Response, S::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut req: http::Request<B>) -> Self::Future {
        let RetryCount(retries) = RetryCount::of(&req);
        let span_context = stamp(self.trace_id, retries, req.headers_mut());
        let response = self.inner.call(req);
        Box::pin(async move {
            let result = response.await;
            match &result {
                Err(error) => log_failed_attempt(&span_context, retries, error),
                Ok(response) if is_failure(response.status()) => {
                    log_failed_attempt(&span_context, retries, response.status())
                }
                Ok(_) => {}
            }
            result
        })
    }
}

/// Whether a response with this status is a failed attempt, worth logging for support
pub fn is_failure(status: http::StatusCode) -> bool {
    status.is_client_error() || status.is_server_error()
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashSet,
        sync::{Arc, Mutex},
        task::Poll,
        time::Duration,
    };

    use futures::future;
    use http::{HeaderMap, HeaderValue, StatusCode};
    use rover_tower::{
        poll_retry::{poll_until_complete, SimplePollOutcome},
        test::MockCloneService,
    };
    use rstest::{fixture, rstest};
    use speculoos::prelude::*;
    use tower::{
        retry::{Policy, RetryLayer},
        ServiceBuilder, ServiceExt,
    };
    use tracing_test::traced_test;

    use super::{
        new_trace_id, process_trace_id, stamp, TraceId, TraceparentLayer, TraceparentService,
    };
    use crate::{
        retry::{RetryCount, RetryPolicy},
        test::MockHttpService,
        Full, HttpRequest, HttpResponse, HttpServiceError,
    };

    /// The headers the inner service saw, in order
    type Seen = Arc<Mutex<Vec<HeaderMap>>>;

    type Traced = TraceparentService<MockCloneService<MockHttpService>>;

    /// A [`TraceparentLayer`] over a mock service that records each request's headers
    fn traced(trace_id: TraceId) -> (Traced, Seen) {
        let seen = Seen::default();
        let recorder = Arc::clone(&seen);
        let mut mock = MockHttpService::new();
        mock.expect_poll_ready().returning(|_| Poll::Ready(Ok(())));
        mock.expect_call().returning(move |req| {
            recorder.lock().unwrap().push(req.headers().clone());
            future::ready(Ok(HttpResponse::default()))
        });
        let service = ServiceBuilder::new()
            .layer(TraceparentLayer::new(trace_id))
            .service(MockCloneService::new(mock));
        (service, seen)
    }

    /// A `traceparent` header value, with its parts named
    struct Traceparent {
        value: String,
        version: String,
        trace_id: String,
        parent_id: String,
        flags: String,
    }

    impl Traceparent {
        /// The only `traceparent` in `headers`
        fn of(headers: &HeaderMap) -> Traceparent {
            let mut values = headers.get_all("traceparent").iter();
            let value = values
                .next()
                .expect("a traceparent header")
                .to_str()
                .unwrap()
                .to_string();
            assert_that!(values.next()).is_none();
            let parts: Vec<&str> = value.split('-').collect();
            assert_that!(&parts).has_length(4);
            Traceparent {
                version: parts[0].to_string(),
                trace_id: parts[1].to_string(),
                parent_id: parts[2].to_string(),
                flags: parts[3].to_string(),
                value,
            }
        }
    }

    #[fixture]
    fn trace_id() -> TraceId {
        new_trace_id()
    }

    #[fixture]
    fn request() -> HttpRequest {
        http::Request::builder()
            .uri("http://example.com")
            .body(Full::default())
            .unwrap()
    }

    #[rstest]
    #[tokio::test]
    async fn a_request_carries_a_valid_traceparent(trace_id: TraceId, request: HttpRequest) {
        let (service, seen) = traced(trace_id);
        service.oneshot(request).await.unwrap();

        let seen = seen.lock().unwrap();
        let traceparent = Traceparent::of(&seen[0]);
        assert_that!(traceparent.version.as_str()).is_equal_to("00");
        assert_that!(traceparent.trace_id).is_equal_to(trace_id.to_string());
        assert_that!(traceparent.parent_id.len()).is_equal_to(16);
        assert_that!(traceparent
            .parent_id
            .chars()
            .all(|c| matches!(c, '0'..='9' | 'a'..='f')))
        .is_true();
        assert_that!(traceparent.parent_id.as_str()).is_not_equal_to("0".repeat(16).as_str());
        assert_that!(traceparent.flags.as_str()).is_equal_to("01");
        assert_that!(traceparent.value.as_str())
            .is_equal_to(format!("00-{trace_id}-{}-01", traceparent.parent_id).as_str());
        assert_that!(seen[0].get("tracestate"))
            .is_equal_to(Some(&HeaderValue::from_static("rover=retries:0")));
    }

    #[rstest]
    #[tokio::test]
    async fn every_request_shares_a_trace_id_and_has_its_own_parent_id(
        trace_id: TraceId,
        request: HttpRequest,
    ) {
        let (service, seen) = traced(trace_id);
        for _ in 0..50 {
            service.clone().oneshot(request.clone()).await.unwrap();
        }

        let seen = seen.lock().unwrap();
        let traceparents: Vec<_> = seen.iter().map(Traceparent::of).collect();
        let trace_ids: HashSet<_> = traceparents.iter().map(|t| t.trace_id.clone()).collect();
        let parent_ids: HashSet<_> = traceparents.iter().map(|t| t.parent_id.clone()).collect();
        assert_that!(trace_ids).is_equal_to(HashSet::from([trace_id.to_string()]));
        assert_that!(parent_ids).has_length(50);
    }

    #[rstest]
    #[tokio::test]
    async fn a_traceparent_and_tracestate_the_request_already_had_are_replaced(
        trace_id: TraceId,
        mut request: HttpRequest,
    ) {
        let (service, seen) = traced(trace_id);
        let stale = "00-00000000000000000000000000000001-0000000000000002-00";
        request
            .headers_mut()
            .insert("traceparent", HeaderValue::from_static(stale));
        request
            .headers_mut()
            .insert("tracestate", HeaderValue::from_static("other=vendor"));
        service.oneshot(request).await.unwrap();

        let seen = seen.lock().unwrap();
        let traceparent = Traceparent::of(&seen[0]);
        assert_that!(traceparent.value.as_str()).is_not_equal_to(stale);
        assert_that!(traceparent.trace_id).is_equal_to(trace_id.to_string());
        assert_that!(seen[0].get_all("tracestate").iter().collect::<Vec<_>>())
            .is_equal_to(vec![&HeaderValue::from_static("rover=retries:0")]);
    }

    #[rstest]
    #[tokio::test]
    async fn the_retry_count_on_a_request_is_the_retries_it_says(
        trace_id: TraceId,
        mut request: HttpRequest,
    ) {
        let (service, seen) = traced(trace_id);
        request.extensions_mut().insert(RetryCount(7));
        service.oneshot(request).await.unwrap();

        let seen = seen.lock().unwrap();
        assert_that!(seen[0].get_all("tracestate").iter().collect::<Vec<_>>())
            .is_equal_to(vec![&HeaderValue::from_static("rover=retries:7")]);
    }

    /// A service that answers each call with the next of `statuses`, recording the headers
    fn flaky(statuses: Vec<StatusCode>) -> (MockCloneService<MockHttpService>, Seen) {
        let seen = Seen::default();
        let recorder = Arc::clone(&seen);
        let statuses = Arc::new(Mutex::new(statuses.into_iter()));
        let mut mock = MockHttpService::new();
        mock.expect_poll_ready().returning(|_| Poll::Ready(Ok(())));
        mock.expect_call().returning(move |req| {
            recorder.lock().unwrap().push(req.headers().clone());
            let status = statuses
                .lock()
                .unwrap()
                .next()
                .expect("an expected attempt");
            let mut response = HttpResponse::default();
            *response.status_mut() = status;
            future::ready(Ok(response))
        });
        (MockCloneService::new(mock), seen)
    }

    fn tracestates(seen: &Seen) -> Vec<String> {
        seen.lock()
            .unwrap()
            .iter()
            .map(|headers| {
                headers
                    .get("tracestate")
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .to_string()
            })
            .collect()
    }

    #[rstest]
    #[tokio::test]
    async fn each_retry_says_how_many_attempts_came_before_it(
        trace_id: TraceId,
        request: HttpRequest,
    ) {
        let (inner, seen) = flaky(vec![
            StatusCode::INTERNAL_SERVER_ERROR,
            StatusCode::SERVICE_UNAVAILABLE,
            StatusCode::OK,
        ]);
        let service = ServiceBuilder::new()
            .layer(RetryLayer::new(RetryPolicy::new(Duration::from_secs(30))))
            .layer(TraceparentLayer::new(trace_id))
            .service(inner);

        let response = service.oneshot(request).await.unwrap();

        assert_that!(response.status()).is_equal_to(StatusCode::OK);
        assert_that!(tracestates(&seen)).is_equal_to(vec![
            "rover=retries:0".to_string(),
            "rover=retries:1".to_string(),
            "rover=retries:2".to_string(),
        ]);
    }

    // The last attempt's failure is what comes back, and the counts it sent had no gaps.
    #[rstest]
    #[tokio::test]
    async fn the_retry_count_keeps_counting_until_retries_run_out(
        trace_id: TraceId,
        request: HttpRequest,
    ) {
        let (inner, seen) = flaky(vec![StatusCode::INTERNAL_SERVER_ERROR; 50]);
        let service = ServiceBuilder::new()
            .layer(RetryLayer::new(RetryPolicy::new(Duration::from_millis(
                100,
            ))))
            .layer(TraceparentLayer::new(trace_id))
            .service(inner);

        let response = service.oneshot(request).await.unwrap();

        assert_that!(response.status()).is_equal_to(StatusCode::INTERNAL_SERVER_ERROR);
        let sent = tracestates(&seen);
        assert_that!(sent.len() >= 2).is_true();
        let expected: Vec<String> = (0..sent.len())
            .map(|retries| format!("rover=retries:{retries}"))
            .collect();
        assert_that!(sent).is_equal_to(expected);
    }

    /// A policy that retries a failed attempt once, and raises the retry count as it should
    #[derive(Clone, Default)]
    struct RetryOnce {
        retried: bool,
    }

    impl Policy<HttpRequest, HttpResponse, HttpServiceError> for RetryOnce {
        type Future = future::Ready<()>;

        fn retry(
            &mut self,
            req: &mut HttpRequest,
            result: &mut Result<HttpResponse, HttpServiceError>,
        ) -> Option<Self::Future> {
            let failed = result
                .as_ref()
                .map_or(true, |response| response.status().is_server_error());
            if self.retried || !failed {
                return None;
            }
            self.retried = true;
            let RetryCount(retries) = RetryCount::of(req);
            req.extensions_mut().insert(RetryCount(retries + 1));
            Some(future::ready(()))
        }

        fn clone_request(&mut self, req: &HttpRequest) -> Option<HttpRequest> {
            Some(req.clone())
        }
    }

    // One service retrying inside another doesn't give a running total, as `RetryCount` says: the
    // outer service's retry starts the inner one's count again from the outer one's.
    #[rstest]
    #[tokio::test]
    async fn services_that_retry_inside_one_another_do_not_share_a_running_total(
        trace_id: TraceId,
        request: HttpRequest,
    ) {
        let (inner, seen) = flaky(vec![
            StatusCode::INTERNAL_SERVER_ERROR,
            StatusCode::INTERNAL_SERVER_ERROR,
            StatusCode::INTERNAL_SERVER_ERROR,
            StatusCode::OK,
        ]);
        let service = ServiceBuilder::new()
            .layer(RetryLayer::new(RetryPolicy::new(Duration::from_secs(30))))
            .layer(RetryLayer::new(RetryOnce::default()))
            .layer(TraceparentLayer::new(trace_id))
            .service(inner);

        let response = service.oneshot(request).await.unwrap();

        assert_that!(response.status()).is_equal_to(StatusCode::OK);
        assert_that!(tracestates(&seen)).is_equal_to(vec![
            "rover=retries:0".to_string(),
            "rover=retries:1".to_string(),
            "rover=retries:1".to_string(),
            "rover=retries:2".to_string(),
        ]);
    }

    #[rstest]
    #[tokio::test]
    async fn the_next_request_starts_again_from_zero(trace_id: TraceId, request: HttpRequest) {
        let (inner, seen) = flaky(vec![
            StatusCode::INTERNAL_SERVER_ERROR,
            StatusCode::OK,
            StatusCode::OK,
        ]);
        let service = ServiceBuilder::new()
            .layer(RetryLayer::new(RetryPolicy::new(Duration::from_secs(30))))
            .layer(TraceparentLayer::new(trace_id))
            .service(inner);

        service.clone().oneshot(request.clone()).await.unwrap();
        service.oneshot(request).await.unwrap();

        assert_that!(tracestates(&seen)).is_equal_to(vec![
            "rover=retries:0".to_string(),
            "rover=retries:1".to_string(),
            "rover=retries:0".to_string(),
        ]);
    }

    // Asking again to see whether something has finished is a new request each time, not a
    // retry, so it starts again from zero however many retries an earlier poll needed.
    #[rstest]
    #[tokio::test]
    async fn each_poll_starts_again_from_zero(trace_id: TraceId, request: HttpRequest) {
        let (inner, seen) = flaky(vec![
            StatusCode::ACCEPTED,
            StatusCode::INTERNAL_SERVER_ERROR,
            StatusCode::ACCEPTED,
            StatusCode::OK,
        ]);
        let platform_api = ServiceBuilder::new()
            .map_response(|response: HttpResponse| {
                if response.status() == StatusCode::OK {
                    SimplePollOutcome::Complete
                } else {
                    SimplePollOutcome::Incomplete
                }
            })
            .layer(RetryLayer::new(RetryPolicy::new(Duration::from_secs(30))))
            .layer(TraceparentLayer::new(trace_id))
            .service(inner);
        let service = poll_until_complete(
            platform_api,
            Duration::from_millis(10),
            Duration::from_secs(30),
            || HttpServiceError::TimedOut,
        );

        service.oneshot(request).await.unwrap();

        // The first poll is answered. The second is retried once, and is answered. The third
        // is a new request.
        assert_that!(tracestates(&seen)).is_equal_to(vec![
            "rover=retries:0".to_string(),
            "rover=retries:0".to_string(),
            "rover=retries:1".to_string(),
            "rover=retries:0".to_string(),
        ]);
    }

    #[traced_test]
    #[rstest]
    #[tokio::test]
    async fn the_trace_id_is_logged_once_with_the_first_request(
        trace_id: TraceId,
        request: HttpRequest,
    ) {
        let (service, _) = traced(trace_id);
        for _ in 0..3 {
            service.clone().oneshot(request.clone()).await.unwrap();
        }

        logs_assert(|lines: &[&str]| {
            let logged = lines
                .iter()
                .filter(|line| line.contains(&format!("carry trace ID {trace_id}")))
                .count();
            match logged {
                1 => Ok(()),
                other => Err(format!(
                    "expected the trace ID once, found it {other} times"
                )),
            }
        });
    }

    #[traced_test]
    #[rstest]
    #[tokio::test]
    async fn a_failed_attempt_is_logged_with_its_ids_and_retry_count(
        trace_id: TraceId,
        request: HttpRequest,
    ) {
        let (inner, seen) = flaky(vec![StatusCode::INTERNAL_SERVER_ERROR, StatusCode::OK]);
        let service = ServiceBuilder::new()
            .layer(RetryLayer::new(RetryPolicy::new(Duration::from_secs(30))))
            .layer(TraceparentLayer::new(trace_id))
            .service(inner);

        service.oneshot(request).await.unwrap();

        let failed_parent_id = Traceparent::of(&seen.lock().unwrap()[0]).parent_id;
        let succeeded_parent_id = Traceparent::of(&seen.lock().unwrap()[1]).parent_id;
        logs_assert(|lines: &[&str]| {
            let failures: Vec<_> = lines
                .iter()
                .filter(|line| line.contains("request attempt failed"))
                .collect();
            let [failure] = failures.as_slice() else {
                return Err(format!("expected one failed attempt, found {failures:?}"));
            };
            for expected in [
                format!("trace_id={trace_id}"),
                format!("parent_id={failed_parent_id}"),
                "retries=0".to_string(),
                "500 Internal Server Error".to_string(),
            ] {
                if !failure.contains(&expected) {
                    return Err(format!("{failure} lacks {expected}"));
                }
            }
            if failure.contains(&succeeded_parent_id) {
                return Err(format!("{failure} names the attempt that succeeded"));
            }
            Ok(())
        });
    }

    #[traced_test]
    #[rstest]
    #[tokio::test]
    async fn a_successful_attempt_is_not_logged_as_failed(trace_id: TraceId, request: HttpRequest) {
        let (service, _) = traced(trace_id);

        service.oneshot(request).await.unwrap();

        assert!(!logs_contain("request attempt failed"));
    }

    #[rstest]
    #[tokio::test]
    async fn a_response_that_is_not_retried_is_sent_once_with_no_retries(
        trace_id: TraceId,
        request: HttpRequest,
    ) {
        let (inner, seen) = flaky(vec![StatusCode::NOT_FOUND]);
        let service = ServiceBuilder::new()
            .layer(RetryLayer::new(RetryPolicy::new(Duration::from_secs(30))))
            .layer(TraceparentLayer::new(trace_id))
            .service(inner);

        service.oneshot(request).await.unwrap();

        assert_that!(tracestates(&seen)).is_equal_to(vec!["rover=retries:0".to_string()]);
    }

    #[rstest]
    fn stamping_names_the_ids_it_sent(trace_id: TraceId) {
        let mut headers = HeaderMap::new();

        let span_context = stamp(trace_id, 2, &mut headers);

        assert_that!(span_context.trace_id()).is_equal_to(trace_id);
        assert_that!(headers.get("tracestate").unwrap().to_str().unwrap())
            .is_equal_to("rover=retries:2");
        assert_that!(headers.get("traceparent").unwrap().to_str().unwrap())
            .is_equal_to(format!("00-{trace_id}-{}-01", span_context.span_id()).as_str());
    }

    #[test]
    fn the_process_has_one_trace_id() {
        assert_that!(process_trace_id()).is_equal_to(process_trace_id());
    }

    #[test]
    fn new_trace_ids_are_distinct() {
        let ids: HashSet<_> = (0..1000).map(|_| new_trace_id()).collect();
        assert_that!(ids).has_length(1000);
    }
}

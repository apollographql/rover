//! Adds a W3C Trace Context `traceparent` header to HTTP requests
//!
//! Trace IDs, span IDs and the header's wire format come from OpenTelemetry, which is the source
//! of truth for them. What is Rover's own is when they are made: one trace ID for the whole
//! process, and a new span ID, which the header calls the parent ID, for each attempt.
//! [`TraceparentLayer`] does that for [`tower`] services, so place it _inside_ any retry layer to
//! give every attempt of a request its own parent ID and the same trace ID. [`stamp`] does it for
//! a client that retries outside of one.

use std::{sync::OnceLock, task::Context};

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

/// Sets `traceparent` in `headers` for one more request in the trace `trace_id`.
///
/// The header carries `trace_id`, a new random span ID as its parent ID, and the sampled flag.
/// Call it once per attempt of a request, and use the returned [`SpanContext`] to say which IDs
/// that attempt sent.
///
/// A `traceparent` the headers already had is replaced. So is a `tracestate`, which belongs to a
/// different trace, and none is sent in its place: W3C says to avoid sending an empty one.
pub fn stamp(trace_id: TraceId, headers: &mut HeaderMap) -> SpanContext {
    let span_context = SpanContext::new(
        trace_id,
        RandomIdGenerator::default().new_span_id(),
        TraceFlags::SAMPLED,
        true,
        TraceState::default(),
    );
    let context = OtelContext::new().with_remote_span_context(span_context.clone());
    TraceContextPropagator::new().inject_context(&context, &mut HeaderInjector(headers));
    headers.remove(TRACESTATE);
    span_context
}

/// Layer that applies [`TraceparentService`], which adds a `traceparent` header to HTTP requests
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

/// Middleware that adds a `traceparent` header to every HTTP request it sends, as [`stamp`] does
#[derive(Clone, Debug)]
pub struct TraceparentService<S> {
    trace_id: TraceId,
    inner: S,
}

impl<S, B> Service<http::Request<B>> for TraceparentService<S>
where
    S: Service<http::Request<B>>,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut req: http::Request<B>) -> Self::Future {
        stamp(self.trace_id, req.headers_mut());
        self.inner.call(req)
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashSet,
        sync::{Arc, Mutex},
        task::Poll,
    };

    use futures::future;
    use http::{HeaderMap, HeaderValue};
    use rover_tower::test::MockCloneService;
    use rstest::{fixture, rstest};
    use speculoos::prelude::*;
    use tower::{ServiceBuilder, ServiceExt};

    use super::{
        new_trace_id, process_trace_id, stamp, TraceId, TraceparentLayer, TraceparentService,
    };
    use crate::{test::MockHttpService, Full, HttpRequest, HttpResponse};

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

    /// The only `traceparent` a request carried
    fn only_traceparent(headers: &HeaderMap) -> &HeaderValue {
        let mut values = headers.get_all("traceparent").iter();
        let value = values.next().expect("a traceparent header");
        assert_that!(values.next()).is_none();
        value
    }

    /// The fields of a `traceparent` value: version, trace ID, parent ID, flags
    fn fields(value: &HeaderValue) -> Vec<String> {
        value
            .to_str()
            .unwrap()
            .split('-')
            .map(str::to_string)
            .collect()
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
        let value = only_traceparent(&seen[0]);
        let parent_id = fields(value)[2].clone();
        assert_that!(value.to_str().unwrap())
            .is_equal_to(format!("00-{trace_id}-{parent_id}-01").as_str());
        assert_that!(parent_id.len()).is_equal_to(16);
        assert_that!(parent_id.as_str()).is_not_equal_to("0".repeat(16).as_str());
        assert_that!(seen[0].get("tracestate")).is_none();
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
        let fields: Vec<_> = seen.iter().map(|h| fields(only_traceparent(h))).collect();
        let trace_ids: HashSet<_> = fields.iter().map(|f| f[1].clone()).collect();
        let parent_ids: HashSet<_> = fields.iter().map(|f| f[2].clone()).collect();
        assert_that!(trace_ids).is_equal_to(HashSet::from([trace_id.to_string()]));
        assert_that!(parent_ids).has_length(50);
    }

    #[rstest]
    #[tokio::test]
    async fn a_clone_of_the_service_sends_the_same_trace_id_with_its_own_parent_id(
        trace_id: TraceId,
        request: HttpRequest,
    ) {
        let (service, seen) = traced(trace_id);
        service.clone().oneshot(request.clone()).await.unwrap();
        service.oneshot(request).await.unwrap();

        let seen = seen.lock().unwrap();
        let first = fields(only_traceparent(&seen[0]));
        let second = fields(only_traceparent(&seen[1]));
        assert_that!(first[1].clone()).is_equal_to(trace_id.to_string());
        assert_that!(second[1].clone()).is_equal_to(trace_id.to_string());
        assert_that!(first[2].clone()).is_not_equal_to(second[2].clone());
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
        let value = only_traceparent(&seen[0]);
        assert_that!(value.to_str().unwrap()).is_not_equal_to(stale);
        assert_that!(fields(value)[1].clone()).is_equal_to(trace_id.to_string());
        assert_that!(seen[0].get("tracestate")).is_none();
    }

    #[rstest]
    fn stamping_names_the_ids_it_sent(trace_id: TraceId) {
        let mut headers = HeaderMap::new();

        let span_context = stamp(trace_id, &mut headers);

        assert_that!(span_context.trace_id()).is_equal_to(trace_id);
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

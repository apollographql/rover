//! Adds a W3C Trace Context `traceparent` header to HTTP requests
//!
//! A [`TraceId`] identifies one trace, and every request sent with the same one belongs to
//! it. [`TraceparentLayer`] stamps each request it sees with that trace ID and a new random
//! parent ID, so place it _inside_ any retry layer to give every attempt of a request its own
//! parent ID and the same trace ID.

use std::{fmt, sync::OnceLock, task::Context};

use http::{HeaderName, HeaderValue};
use tower::{Layer, Service};

const TRACEPARENT: HeaderName = HeaderName::from_static("traceparent");

/// The only `traceparent` version the W3C Trace Context recommendation defines
const VERSION: &str = "00";

/// The trace flags with only the "sampled" flag set
const SAMPLED: &str = "01";

/// The 16-byte identifier shared by every request in one trace.
///
/// Always non-zero, as the W3C Trace Context recommendation requires.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TraceId(u128);

impl TraceId {
    /// Generates a random trace ID.
    ///
    /// It comes straight from a cryptographically secure random source and is derived from
    /// nothing else, so it can't be tied back to a session, a machine, or a person.
    pub fn generate() -> TraceId {
        loop {
            let value: u128 = rand::random();
            if value != 0 {
                return TraceId(value);
            }
        }
    }

    /// The trace ID for this process, generated the first time it is asked for.
    ///
    /// Rover is a one-shot command, so a process is an invocation: every Platform API request
    /// in one run shares this ID, however many clients it builds along the way.
    pub fn for_process() -> TraceId {
        static PROCESS: OnceLock<TraceId> = OnceLock::new();
        *PROCESS.get_or_init(TraceId::generate)
    }

    /// The `traceparent` for one more request in this trace, with a new random parent ID.
    ///
    /// Call it once per attempt of a request. [`TraceparentLayer`] does that for [`tower`]
    /// services; this is for clients that retry outside of one.
    pub fn traceparent(&self) -> Traceparent {
        Traceparent {
            trace_id: *self,
            parent_id: ParentId::generate(),
        }
    }
}

impl fmt::Display for TraceId {
    /// Writes the ID as 32 lowercase hexadecimal digits
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:032x}", self.0)
    }
}

/// The 8-byte identifier of one attempt of a request within a trace.
///
/// Always non-zero, as the W3C Trace Context recommendation requires.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ParentId(u64);

impl ParentId {
    fn generate() -> ParentId {
        loop {
            let value: u64 = rand::random();
            if value != 0 {
                return ParentId(value);
            }
        }
    }
}

impl fmt::Display for ParentId {
    /// Writes the ID as 16 lowercase hexadecimal digits
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:016x}", self.0)
    }
}

/// The value of a `traceparent` header: which trace a request belongs to, and which attempt of
/// it this is.
///
/// Its [`Display`](fmt::Display) form is the header value, so a client that logs an attempt can
/// name the same IDs it sent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Traceparent {
    trace_id: TraceId,
    parent_id: ParentId,
}

impl Traceparent {
    /// The trace this request belongs to
    pub const fn trace_id(&self) -> TraceId {
        self.trace_id
    }

    /// The attempt of the request this is
    pub const fn parent_id(&self) -> ParentId {
        self.parent_id
    }

    /// The header value to send
    pub fn header_value(&self) -> HeaderValue {
        HeaderValue::from_str(&self.to_string())
            .expect("a traceparent is made only of hexadecimal digits and dashes")
    }
}

impl fmt::Display for Traceparent {
    /// Writes `00-<trace id>-<parent id>-01`
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{VERSION}-{}-{}-{SAMPLED}",
            self.trace_id, self.parent_id
        )
    }
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

/// Middleware that adds a `traceparent` header to every HTTP request it sends
///
/// Each request gets the same trace ID, a new random parent ID, and the sampled flag. A
/// `traceparent` the request already had is replaced.
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
        req.headers_mut()
            .insert(TRACEPARENT, self.trace_id.traceparent().header_value());
        self.inner.call(req)
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashSet,
        convert::Infallible,
        sync::{Arc, Mutex},
    };

    use http::{HeaderValue, Request};
    use rstest::{fixture, rstest};
    use speculoos::prelude::*;
    use tower::{service_fn, Service, ServiceBuilder, ServiceExt};

    use super::{ParentId, TraceId, TraceparentLayer};

    /// The headers a service saw, in order
    type Seen = Arc<Mutex<Vec<http::HeaderMap>>>;

    /// A service stacked under [`TraceparentLayer`] that records each request's headers
    fn traced(trace_id: TraceId) -> (impl Service<Request<()>, Error = Infallible> + Clone, Seen) {
        let seen = Seen::default();
        let recorder = Arc::clone(&seen);
        let service = ServiceBuilder::new()
            .layer(TraceparentLayer::new(trace_id))
            .service(service_fn(move |req: Request<()>| {
                let recorder = Arc::clone(&recorder);
                async move {
                    recorder.lock().unwrap().push(req.headers().clone());
                    Ok::<_, Infallible>(())
                }
            }));
        (service, seen)
    }

    async fn send(service: &mut (impl Service<Request<()>, Error = Infallible> + Clone)) {
        let request = Request::builder()
            .uri("http://example.com")
            .body(())
            .unwrap();
        service.ready().await.unwrap().call(request).await.unwrap();
    }

    /// The only `traceparent` a request carried
    fn only_traceparent(headers: &http::HeaderMap) -> &HeaderValue {
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

    fn is_lowercase_hex(value: &str, length: usize) -> bool {
        value.len() == length
            && value
                .chars()
                .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c))
    }

    #[fixture]
    fn trace_id() -> TraceId {
        TraceId::generate()
    }

    #[rstest]
    #[tokio::test]
    async fn a_request_carries_a_valid_traceparent(trace_id: TraceId) {
        let (mut service, seen) = traced(trace_id);
        send(&mut service).await;

        let seen = seen.lock().unwrap();
        let value = only_traceparent(&seen[0]);
        let parent_id = fields(value)[2].clone();
        assert_that!(is_lowercase_hex(&parent_id, 16)).is_true();
        assert_that!(parent_id.as_str()).is_not_equal_to("0".repeat(16).as_str());
        assert_that!(value.to_str().unwrap())
            .is_equal_to(format!("00-{trace_id}-{parent_id}-01").as_str());
        assert_that!(is_lowercase_hex(&trace_id.to_string(), 32)).is_true();
        assert_that!(trace_id.to_string().as_str()).is_not_equal_to("0".repeat(32).as_str());
    }

    #[rstest]
    #[tokio::test]
    async fn every_request_shares_a_trace_id_and_has_its_own_parent_id(trace_id: TraceId) {
        let (mut service, seen) = traced(trace_id);
        for _ in 0..50 {
            send(&mut service).await;
        }

        let seen = seen.lock().unwrap();
        let fields: Vec<_> = seen.iter().map(|h| fields(only_traceparent(h))).collect();
        let trace_ids: HashSet<_> = fields.iter().map(|f| f[1].clone()).collect();
        let parent_ids: HashSet<_> = fields.iter().map(|f| f[2].clone()).collect();
        assert_that!(trace_ids).is_equal_to(HashSet::from([trace_id.to_string()]));
        assert_that!(parent_ids.len()).is_equal_to(50);
    }

    #[rstest]
    #[tokio::test]
    async fn a_clone_of_the_service_sends_the_same_trace_id_with_its_own_parent_id(
        trace_id: TraceId,
    ) {
        let (mut service, seen) = traced(trace_id);
        let mut clone = service.clone();
        send(&mut service).await;
        send(&mut clone).await;

        let seen = seen.lock().unwrap();
        let first = fields(only_traceparent(&seen[0]));
        let second = fields(only_traceparent(&seen[1]));
        assert_that!(first[1].clone()).is_equal_to(trace_id.to_string());
        assert_that!(second[1].clone()).is_equal_to(trace_id.to_string());
        assert_that!(first[2].clone()).is_not_equal_to(second[2].clone());
    }

    #[rstest]
    #[tokio::test]
    async fn a_traceparent_the_request_already_had_is_replaced(trace_id: TraceId) {
        let (mut service, seen) = traced(trace_id);
        let stale = "00-00000000000000000000000000000001-0000000000000002-00";
        let request = Request::builder()
            .uri("http://example.com")
            .header("traceparent", stale)
            .body(())
            .unwrap();
        service.ready().await.unwrap().call(request).await.unwrap();

        let seen = seen.lock().unwrap();
        let value = only_traceparent(&seen[0]);
        assert_that!(value.to_str().unwrap()).is_not_equal_to(stale);
        assert_that!(fields(value)[1].clone()).is_equal_to(trace_id.to_string());
    }

    #[rstest]
    fn each_traceparent_has_the_trace_id_and_its_own_parent_id(trace_id: TraceId) {
        let first = trace_id.traceparent();
        let second = trace_id.traceparent();

        assert_that!(first.trace_id()).is_equal_to(trace_id);
        assert_that!(second.trace_id()).is_equal_to(trace_id);
        assert_that!(first.parent_id()).is_not_equal_to(second.parent_id());
    }

    #[rstest]
    fn a_traceparent_is_its_header_value_and_names_its_ids(trace_id: TraceId) {
        let traceparent = trace_id.traceparent();
        let expected = format!("00-{trace_id}-{}-01", traceparent.parent_id());

        assert_that!(traceparent.to_string()).is_equal_to(expected.clone());
        assert_that!(traceparent.header_value())
            .is_equal_to(HeaderValue::from_str(&expected).unwrap());
    }

    #[test]
    fn the_process_has_one_trace_id() {
        assert_that!(TraceId::for_process()).is_equal_to(TraceId::for_process());
    }

    #[test]
    fn generated_trace_ids_are_distinct() {
        let ids: HashSet<_> = (0..1000).map(|_| TraceId::generate()).collect();
        assert_that!(ids.len()).is_equal_to(1000);
    }

    #[test]
    fn a_trace_id_is_written_as_32_lowercase_hex_digits() {
        assert_that!(TraceId(0xAB).to_string())
            .is_equal_to("000000000000000000000000000000ab".to_string());
    }

    #[test]
    fn a_parent_id_is_written_as_16_lowercase_hex_digits() {
        assert_that!(ParentId(0xAB).to_string()).is_equal_to("00000000000000ab".to_string());
    }
}

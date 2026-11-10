//! Adds a W3C Trace Context `traceparent` header to HTTP requests
//!
//! A [`TraceId`] identifies one trace, and every request sent with the same one belongs to
//! it. [`TraceparentLayer`] stamps each request it sees with that trace ID and a new random
//! parent ID, so place it _inside_ any retry layer to give every attempt of a request its own
//! parent ID and the same trace ID.

use std::{fmt, task::Context};

use http::{header::InvalidHeaderValue, HeaderName, HeaderValue};
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
        TraceId(random_trace_id())
    }
}

impl TraceId {
    /// A `traceparent` header value for one more request in this trace.
    ///
    /// It carries this trace ID, a new random non-zero parent ID, and the sampled flag, so call
    /// it once per attempt of a request. [`TraceparentLayer`] does that for [`tower`] services;
    /// this is for clients that retry outside of one.
    pub fn traceparent(&self) -> Result<HeaderValue, InvalidHeaderValue> {
        HeaderValue::try_from(format!(
            "{VERSION}-{self}-{:016x}-{SAMPLED}",
            random_parent_id()
        ))
    }
}

impl fmt::Display for TraceId {
    /// Writes the ID as 32 lowercase hexadecimal digits
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:032x}", self.0)
    }
}

fn random_trace_id() -> u128 {
    loop {
        let value: u128 = rand::random();
        if value != 0 {
            return value;
        }
    }
}

fn random_parent_id() -> u64 {
    loop {
        let value: u64 = rand::random();
        if value != 0 {
            return value;
        }
    }
}

/// Layer that applies [`Traceparent`], which adds a `traceparent` header to HTTP requests
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
    type Service = Traceparent<S>;

    fn layer(&self, inner: S) -> Self::Service {
        Traceparent {
            trace_id: self.trace_id,
            inner,
        }
    }
}

/// Middleware that adds a `traceparent` header to every HTTP request it sends
///
/// Each request gets the same trace ID and a new random parent ID, and the sampled flag. A
/// `traceparent` the request already had is replaced.
///
/// A failure to build the header never fails or changes the request: it is sent without one,
/// and the failure is logged at debug level.
#[derive(Clone, Debug)]
pub struct Traceparent<S> {
    trace_id: TraceId,
    inner: S,
}

impl<S, B> Service<http::Request<B>> for Traceparent<S>
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
        match self.trace_id.traceparent() {
            Ok(value) => {
                req.headers_mut().insert(TRACEPARENT, value);
            }
            Err(err) => tracing::debug!(%err, "could not attach a traceparent header"),
        }
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

    use super::{TraceId, TraceparentLayer};

    /// The `traceparent` values a service saw, in order, `None` for a request without one
    type Seen = Arc<Mutex<Vec<Option<HeaderValue>>>>;

    /// A service stacked under [`TraceparentLayer`] that records each request's `traceparent`
    fn traced(trace_id: TraceId) -> (impl Service<Request<()>, Error = Infallible> + Clone, Seen) {
        let seen = Seen::default();
        let recorder = Arc::clone(&seen);
        let service = ServiceBuilder::new()
            .layer(TraceparentLayer::new(trace_id))
            .service(service_fn(move |req: Request<()>| {
                let recorder = Arc::clone(&recorder);
                async move {
                    recorder
                        .lock()
                        .unwrap()
                        .push(req.headers().get("traceparent").cloned());
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
        let value = seen[0].as_ref().expect("a traceparent header");
        let fields = fields(value);
        assert_that!(fields[0].as_str()).is_equal_to("00");
        assert_that!(fields[1]).is_equal_to(trace_id.to_string());
        assert_that!(is_lowercase_hex(&fields[1], 32)).is_true();
        assert_that!(fields[1].as_str()).is_not_equal_to("0".repeat(32).as_str());
        assert_that!(is_lowercase_hex(&fields[2], 16)).is_true();
        assert_that!(fields[2].as_str()).is_not_equal_to("0".repeat(16).as_str());
        assert_that!(fields[3].as_str()).is_equal_to("01");
    }

    #[rstest]
    #[tokio::test]
    async fn every_request_shares_a_trace_id_and_has_its_own_parent_id(trace_id: TraceId) {
        let (mut service, seen) = traced(trace_id);
        for _ in 0..50 {
            send(&mut service).await;
        }

        let seen = seen.lock().unwrap();
        let fields: Vec<_> = seen.iter().map(|v| fields(v.as_ref().unwrap())).collect();
        let trace_ids: HashSet<_> = fields.iter().map(|f| f[1].clone()).collect();
        let parent_ids: HashSet<_> = fields.iter().map(|f| f[2].clone()).collect();
        assert_that!(trace_ids).is_equal_to(HashSet::from([trace_id.to_string()]));
        assert_that!(parent_ids.len()).is_equal_to(50);
    }

    #[tokio::test]
    async fn a_clone_of_the_service_sends_the_same_trace_id_with_its_own_parent_id() {
        let (mut service, seen) = traced(TraceId::generate());
        let mut clone = service.clone();
        send(&mut service).await;
        send(&mut clone).await;

        let seen = seen.lock().unwrap();
        let first = fields(seen[0].as_ref().unwrap());
        let second = fields(seen[1].as_ref().unwrap());
        assert_that!(first[1]).is_equal_to(second[1].clone());
        assert_that!(first[2]).is_not_equal_to(second[2].clone());
    }

    #[tokio::test]
    async fn a_traceparent_the_request_already_had_is_replaced() {
        let trace_id = TraceId::generate();
        let (mut service, seen) = traced(trace_id);
        let request = Request::builder()
            .uri("http://example.com")
            .header("traceparent", "00-1-2-00")
            .body(())
            .unwrap();
        service.ready().await.unwrap().call(request).await.unwrap();

        let seen = seen.lock().unwrap();
        assert_that!(fields(seen[0].as_ref().unwrap())[1]).is_equal_to(trace_id.to_string());
    }

    #[rstest]
    fn each_traceparent_value_has_the_trace_id_and_its_own_parent_id(trace_id: TraceId) {
        let first = fields(&trace_id.traceparent().unwrap());
        let second = fields(&trace_id.traceparent().unwrap());

        assert_that!(first[1]).is_equal_to(trace_id.to_string());
        assert_that!(second[1]).is_equal_to(trace_id.to_string());
        assert_that!(first[2]).is_not_equal_to(second[2].clone());
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
}

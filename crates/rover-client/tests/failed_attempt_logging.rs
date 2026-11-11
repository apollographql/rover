//! What the blocking client logs when an attempt at a Platform API request fails.
//!
//! This is its own test binary because it installs the process's one log subscriber. A
//! subscriber that a test installs just for itself can lose events when other tests in the same
//! process are logging at the same time.

use std::{
    io,
    sync::{Arc, Mutex},
    time::Duration,
};

use graphql_client::{GraphQLQuery, QueryBody};
use houston::{Credential, CredentialOrigin};
use httpmock::prelude::*;
use reqwest::Client;
use rover_client::blocking::StudioClient;
use rover_http::trace::process_trace_id;
use serde::{Deserialize, Serialize};
use speculoos::prelude::*;
use tracing_subscriber::fmt::MakeWriter;

struct TestQuery;

#[derive(Serialize, Debug)]
struct TestQueryVariables {}

#[derive(Deserialize, Debug)]
struct TestQueryResponse {}

impl GraphQLQuery for TestQuery {
    type Variables = TestQueryVariables;
    type ResponseData = TestQueryResponse;

    fn build_query(variables: Self::Variables) -> QueryBody<Self::Variables> {
        QueryBody {
            variables,
            query: "query Test { __typename }",
            operation_name: "Test",
        }
    }
}

/// Collects what the subscriber writes, so the test can read the debug log
#[derive(Clone, Default)]
struct LogBuffer(Arc<Mutex<Vec<u8>>>);

impl io::Write for LogBuffer {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl MakeWriter<'_> for LogBuffer {
    type Writer = LogBuffer;

    fn make_writer(&self) -> LogBuffer {
        self.clone()
    }
}

// A person gives support the IDs of the attempt that failed, so the log has to name them, and
// has to name only that attempt.
#[tokio::test]
async fn a_failed_attempt_is_logged_with_its_ids_and_retry_count() {
    let log = LogBuffer::default();
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .with_ansi(false)
        .with_writer(log.clone())
        .init();

    let server = MockServer::start();
    let traceparents = Arc::new(Mutex::new(Vec::<String>::new()));
    let recorder = Arc::clone(&traceparents);
    server.mock(|when, then| {
        when.method(POST).path("/graphql");
        then.respond_with(move |req| {
            let mut traceparents = recorder.lock().unwrap();
            traceparents.push(
                req.headers()
                    .get("traceparent")
                    .map(|value| value.to_str().unwrap().to_string())
                    .unwrap_or_default(),
            );
            // The first attempt fails and the retry succeeds
            let (status, body) = if traceparents.len() == 1 {
                (500, "")
            } else {
                (200, r#"{"data":{}}"#)
            };
            http::Response::builder()
                .status(status)
                .header("content-type", "application/json")
                .body(body.to_string())
                .unwrap()
                .into()
        });
    });
    let client = StudioClient::new(
        Credential {
            api_key: "an-api-key".to_string(),
            origin: CredentialOrigin::EnvVar,
            expires_at: None,
        },
        &server.url("/graphql"),
        "test-version",
        false,
        Client::new(),
        Duration::from_secs(10),
    );

    client
        .post::<TestQuery>(TestQueryVariables {})
        .await
        .unwrap();

    let traceparents = traceparents.lock().unwrap().clone();
    let parent_id = |traceparent: &str| traceparent.split('-').nth(2).unwrap().to_string();
    let (failed, succeeded) = (parent_id(&traceparents[0]), parent_id(&traceparents[1]));
    let log = String::from_utf8(log.0.lock().unwrap().clone()).unwrap();
    let failures: Vec<&str> = log
        .lines()
        .filter(|line| line.contains("request attempt failed"))
        .collect();
    assert_that!(failures).has_length(1);
    assert_that!(failures[0]).contains(format!("trace_id={}", process_trace_id()).as_str());
    assert_that!(failures[0]).contains(format!("parent_id={failed}").as_str());
    assert_that!(failures[0]).contains("retries=0");
    assert_that!(failures[0]).contains("500 Internal Server Error");
    assert_that!(failures[0].contains(&succeeded)).is_false();
    assert_that!(log
        .lines()
        .filter(|line| line.contains("carry trace ID"))
        .count())
    .is_equal_to(1);
}

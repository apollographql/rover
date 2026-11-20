use std::{
    io::Write,
    path::{Path, PathBuf},
    time::Duration,
};

use graphql_client::{GraphQLQuery, QueryBody};
use httpmock::{Method::POST, MockServer};
use rover_auth::external_token::{
    ExternalTokenError, ExternalTokenFailure, ExternalTokenOrigin, ExternalTokenSource,
};
use rover_client::{blocking::StudioClient, RoverClientError};
use rover_graphql::{GraphQLRequest, GraphQLService, GraphQLServiceError};
use rover_http::HttpServiceError;
use rover_studio::service::external_auth::ExternalAuthError;
use rstest::{fixture, rstest};
use serde_json::{json, Value};
use speculoos::prelude::*;
use tempfile::{NamedTempFile, TempDir};
use tower::ServiceExt;

const QUERY: &str = "query ExternalQuery { value }";

struct ExternalQuery;

impl GraphQLQuery for ExternalQuery {
    type Variables = Value;
    type ResponseData = Value;

    fn build_query(variables: Value) -> QueryBody<Value> {
        QueryBody {
            variables,
            query: QUERY,
            operation_name: "ExternalQuery",
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum Transport {
    Legacy,
    LegacyNoRetry,
    Tower,
    TowerAttemptTimeout,
    TowerNoRetry,
    AuthenticatedHttp,
}

impl Transport {
    async fn execute(self, client: &StudioClient) -> Result<Value, RoverClientError> {
        let variables = json!({"original": "payload"});
        match self {
            Self::Legacy => client.post::<ExternalQuery>(variables).await,
            Self::LegacyNoRetry => client.post_no_retry::<ExternalQuery>(variables).await,
            _ => {
                let service = match self {
                    Self::Tower => client.studio_graphql_service()?,
                    Self::TowerAttemptTimeout => client
                        .studio_graphql_service_with_attempt_timeout(Duration::from_secs(2))?,
                    Self::TowerNoRetry => {
                        client.studio_graphql_service_with_timeout(Duration::from_secs(2))?
                    }
                    Self::AuthenticatedHttp => GraphQLService::new(
                        None,
                        client.authenticated_http_service(Duration::from_secs(2))?,
                    ),
                    Self::Legacy | Self::LegacyNoRetry => unreachable!(),
                };
                service
                    .oneshot(GraphQLRequest::<ExternalQuery>::new(variables))
                    .await
                    .map_err(RoverClientError::studio_service)
            }
        }
    }
}

struct TokenFile {
    _directory: TempDir,
    path: PathBuf,
}

impl TokenFile {
    fn publish_to(path: &Path, token: &[u8]) {
        let mut replacement = NamedTempFile::new_in(path.parent().unwrap()).unwrap();
        replacement.write_all(token).unwrap();
        let replacement = replacement.into_temp_path();
        std::fs::rename(&replacement, path).unwrap();
    }

    fn publish(&self, token: &[u8]) {
        Self::publish_to(&self.path, token);
    }

    fn source(&self, origin: ExternalTokenOrigin) -> ExternalTokenSource {
        ExternalTokenSource::new(&self.path, origin)
    }

    fn client(&self, server: &MockServer) -> StudioClient {
        self.client_at(&server.url("/graphql"))
    }

    fn client_at(&self, endpoint: &str) -> StudioClient {
        StudioClient::new_external(
            self.source(ExternalTokenOrigin::Option),
            endpoint,
            "test-version",
            true,
            reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .unwrap(),
            Duration::from_secs(2),
        )
    }
}

#[fixture]
fn token_file() -> TokenFile {
    let directory = tempfile::tempdir().unwrap();
    let file = TokenFile {
        path: directory.path().join("access-token"),
        _directory: directory,
    };
    file.publish(b"A");
    file
}

#[rstest]
#[case::legacy(Transport::Legacy)]
#[case::legacy_no_retry(Transport::LegacyNoRetry)]
#[case::tower(Transport::Tower)]
#[case::tower_attempt_timeout(Transport::TowerAttemptTimeout)]
#[case::tower_no_retry(Transport::TowerNoRetry)]
#[case::authenticated_http(Transport::AuthenticatedHttp)]
#[tokio::test]
async fn each_path_reads_replacements_and_can_recover_in_a_later_operation(
    token_file: TokenFile,
    #[case] transport: Transport,
) {
    let server = MockServer::start_async().await;
    let original = server.mock(|when, then| {
        when.method(POST)
            .path("/graphql")
            .header("authorization", "Bearer A")
            .header_missing("x-api-key")
            .header("apollographql-client-name", "rover-client")
            .header("apollographql-client-version", "test-version")
            .header("apollo-sudo", "true")
            .header("tracestate", "rover=retries:0")
            .json_body(json!({
                "query": QUERY,
                "operationName": "ExternalQuery",
                "variables": {"original": "payload"}
            }));
        then.status(200)
            .json_body(json!({"data": {"value": "account-a"}}));
    });
    let replacement = server.mock(|when, then| {
        when.method(POST)
            .path("/graphql")
            .header("authorization", "Bearer B")
            .header_missing("x-api-key")
            .header("tracestate", "rover=retries:0");
        then.status(200)
            .json_body(json!({"data": {"value": "account-b"}}));
    });
    let client = token_file.client(&server);

    let first = transport.execute(&client).await;
    std::fs::remove_file(&token_file.path).unwrap();
    let missing = transport.execute(&client).await;
    token_file.publish(b"B");
    let repaired = transport.execute(&client).await;

    assert_that!(first)
        .is_ok()
        .is_equal_to(json!({"value": "account-a"}));
    let error = assert_that!(missing).is_err().subject;
    assert_that!(ExternalAuthError::find_in_chain(error).cloned())
        .is_some()
        .is_equal_to(ExternalAuthError::Source(ExternalTokenError {
            origin: ExternalTokenOrigin::Option,
            path: token_file.path.clone(),
            kind: ExternalTokenFailure::Missing,
        }));
    assert_that!(repaired)
        .is_ok()
        .is_equal_to(json!({"value": "account-b"}));
    original.assert_calls(1);
    replacement.assert_calls(1);
}

#[rstest]
#[case::legacy(Transport::Legacy)]
#[case::legacy_no_retry(Transport::LegacyNoRetry)]
#[case::tower(Transport::Tower)]
#[case::tower_attempt_timeout(Transport::TowerAttemptTimeout)]
#[case::tower_no_retry(Transport::TowerNoRetry)]
#[case::authenticated_http(Transport::AuthenticatedHttp)]
#[tokio::test]
async fn a_status_rejection_replays_the_same_request_once_with_an_already_published_token(
    token_file: TokenFile,
    #[case] transport: Transport,
    #[values(401, 406)] status: u16,
) {
    let server = MockServer::start_async().await;
    let path = token_file.path.clone();
    let rejected = server.mock(|when, then| {
        when.method(POST)
            .path("/graphql")
            .header("authorization", "Bearer A")
            .header("tracestate", "rover=retries:0");
        then.respond_with(move |_| {
            TokenFile::publish_to(&path, b"B");
            http::Response::builder()
                .status(status)
                .body("")
                .unwrap()
                .into()
        });
    });
    let resent = server.mock(|when, then| {
        when.method(POST)
            .path("/graphql")
            .header("authorization", "Bearer B")
            .header_missing("x-api-key")
            .header("tracestate", "rover=retries:1")
            .json_body(json!({
                "query": QUERY,
                "operationName": "ExternalQuery",
                "variables": {"original": "payload"}
            }));
        then.status(200)
            .json_body(json!({"data": {"value": "accepted"}}));
    });

    let result = transport.execute(&token_file.client(&server)).await;

    assert_that!(result)
        .is_ok()
        .is_equal_to(json!({"value": "accepted"}));
    rejected.assert_calls(1);
    resent.assert_calls(1);
}

#[rstest]
#[case::legacy(Transport::Legacy)]
#[case::legacy_no_retry(Transport::LegacyNoRetry)]
#[case::tower(Transport::Tower)]
#[case::tower_attempt_timeout(Transport::TowerAttemptTimeout)]
#[case::tower_no_retry(Transport::TowerNoRetry)]
#[case::authenticated_http(Transport::AuthenticatedHttp)]
#[tokio::test]
async fn a_body_rejection_is_not_replayed_even_when_the_token_changes(
    token_file: TokenFile,
    #[case] transport: Transport,
    #[values("Unauthorized: Invalid credentials provided", "406: Not Acceptable")] message: &str,
) {
    let server = MockServer::start_async().await;
    let path = token_file.path.clone();
    let body = json!({"data": null, "errors": [{"message": message}]}).to_string();
    let rejected = server.mock(|when, then| {
        when.method(POST)
            .path("/graphql")
            .header("authorization", "Bearer A");
        then.respond_with(move |_| {
            TokenFile::publish_to(&path, b"B");
            http::Response::builder()
                .status(200)
                .body(body.clone())
                .unwrap()
                .into()
        });
    });
    let unexpected = server.mock(|when, then| {
        when.header("authorization", "Bearer B");
        then.status(200)
            .json_body(json!({"data": {"value": "must not replay"}}));
    });

    let result = transport.execute(&token_file.client(&server)).await;

    let error = assert_that!(result).is_err().subject;
    assert_that!(ExternalAuthError::find_in_chain(error).cloned())
        .is_some()
        .is_equal_to(ExternalAuthError::Rejected);
    rejected.assert_calls(1);
    unexpected.assert_calls(0);
}

#[rstest]
#[case::legacy(Transport::Legacy)]
#[case::legacy_no_retry(Transport::LegacyNoRetry)]
#[case::tower(Transport::Tower)]
#[case::tower_attempt_timeout(Transport::TowerAttemptTimeout)]
#[case::tower_no_retry(Transport::TowerNoRetry)]
#[case::authenticated_http(Transport::AuthenticatedHttp)]
#[tokio::test]
async fn a_permission_denial_is_distinct_and_never_replayed(
    token_file: TokenFile,
    #[case] transport: Transport,
) {
    let server = MockServer::start_async().await;
    let denied = server.mock(|when, then| {
        when.method(POST)
            .path("/graphql")
            .header("authorization", "Bearer A");
        then.status(403).body("not an authentication failure");
    });

    let result = transport.execute(&token_file.client(&server)).await;

    let error = assert_that!(result).is_err().subject;
    assert_that!(ExternalAuthError::find_in_chain(error).cloned())
        .is_some()
        .is_equal_to(ExternalAuthError::PermissionDenied);
    denied.assert_calls(1);
}

#[rstest]
#[case::option(
    ExternalTokenOrigin::Option,
    houston::CredentialOrigin::ExternalTokenFileOption
)]
#[case::environment(
    ExternalTokenOrigin::Environment,
    houston::CredentialOrigin::ExternalTokenFileEnvironment
)]
fn construction_and_headers_do_not_read_the_source_or_embed_a_credential(
    token_file: TokenFile,
    #[case] origin: ExternalTokenOrigin,
    #[case] expected: houston::CredentialOrigin,
) {
    std::fs::remove_file(&token_file.path).unwrap();
    let client = StudioClient::new_external(
        token_file.source(origin),
        "https://platform.example/graphql",
        "test-version",
        false,
        reqwest::Client::new(),
        Duration::ZERO,
    );
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert("apollographql-client-name", "rover-client".parse().unwrap());
    headers.insert(
        "apollographql-client-version",
        "test-version".parse().unwrap(),
    );

    assert_that!(client.build_studio_headers())
        .is_ok()
        .is_equal_to(headers);
    assert_that!(client.get_credential_origin()).is_equal_to(expected);
}

#[rstest]
#[case::legacy(Transport::Legacy)]
#[case::tower(Transport::Tower)]
#[case::tower_attempt_timeout(Transport::TowerAttemptTimeout)]
#[tokio::test]
async fn an_eligible_transient_retry_reads_the_replacement_without_an_extra_authentication_attempt(
    token_file: TokenFile,
    #[case] transport: Transport,
) {
    let server = MockServer::start_async().await;
    let path = token_file.path.clone();
    let transient = server.mock(|when, then| {
        when.method(POST)
            .path("/graphql")
            .header("authorization", "Bearer A")
            .header("tracestate", "rover=retries:0");
        then.respond_with(move |_| {
            TokenFile::publish_to(&path, b"B");
            http::Response::builder()
                .status(503)
                .body("")
                .unwrap()
                .into()
        });
    });
    let retried = server.mock(|when, then| {
        when.method(POST)
            .path("/graphql")
            .header("authorization", "Bearer B")
            .header_missing("x-api-key")
            .header("tracestate", "rover=retries:1");
        then.status(200)
            .json_body(json!({"data": {"value": "retried"}}));
    });

    let result = transport.execute(&token_file.client(&server)).await;

    assert_that!(result)
        .is_ok()
        .is_equal_to(json!({"value": "retried"}));
    transient.assert_calls(1);
    retried.assert_calls(1);
}

#[rstest]
#[case::legacy(Transport::LegacyNoRetry)]
#[case::tower(Transport::TowerNoRetry)]
#[case::authenticated_http(Transport::AuthenticatedHttp)]
#[tokio::test]
async fn operations_without_transient_retries_do_not_gain_them(
    token_file: TokenFile,
    #[case] transport: Transport,
) {
    let server = MockServer::start_async().await;
    let transient = server.mock(|when, then| {
        when.method(POST).path("/graphql");
        then.status(503)
            .json_body(json!({"errors": [{"message": "temporary failure"}]}));
    });

    let result = transport.execute(&token_file.client(&server)).await;

    let error = &assert_that!(result).is_err().subject;
    match transport {
        Transport::LegacyNoRetry => {
            let RoverClientError::SendRequest { source, .. } = error else {
                panic!("Expected the HTTP status error, got {error:?}");
            };
            assert_that!(source.status())
                .is_some()
                .is_equal_to(http::StatusCode::SERVICE_UNAVAILABLE);
        }
        _ => {
            let RoverClientError::Service { source, .. } = error else {
                panic!("Expected the GraphQL response error, got {error:?}");
            };
            assert_that!(source.downcast_ref::<GraphQLServiceError<Value>>())
                .is_some()
                .matches(|error| {
                    matches!(error, GraphQLServiceError::NoData(errors)
                        if errors.len() == 1
                            && errors[0].message == "temporary failure"
                            && errors[0].locations.is_none()
                            && errors[0].path.is_none()
                            && errors[0].extensions.is_none())
                });
            assert_that!(source.to_string()).is_equal_to("No data field provided".to_string());
        }
    }
    transient.assert_calls(1);
}

#[rstest]
#[case::legacy_unavailable(Transport::Legacy, 503, true, false)]
#[case::legacy_success(Transport::Legacy, 200, false, false)]
#[case::legacy_rejection(Transport::Legacy, 401, true, false)]
#[case::tower_rejection(Transport::Tower, 406, true, false)]
#[case::legacy_no_retry(Transport::LegacyNoRetry, 503, false, false)]
#[case::legacy_body_timeout(Transport::Legacy, 200, false, true)]
#[tokio::test]
async fn truncated_response_bodies_preserve_status_first_legacy_behavior(
    token_file: TokenFile,
    #[case] transport: Transport,
    #[case] status: u16,
    #[case] expects_second_attempt: bool,
    #[case] slow_body: bool,
) {
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}/graphql", listener.local_addr().unwrap());
    let mut requests = Vec::new();
    // A raw peer is needed: normal HTTP mocks serialize well-formed response bodies.
    let peer = async {
        for (index, status) in [status, 200].into_iter().enumerate() {
            let (mut connection, _) = listener.accept().await?;
            let mut request = Vec::new();
            loop {
                if connection.read_buf(&mut request).await? == 0 {
                    return Err(std::io::Error::other("incomplete request"));
                }
                if let Some(end) = request.windows(4).position(|bytes| bytes == b"\r\n\r\n") {
                    let headers = String::from_utf8_lossy(&request[..end]);
                    let length = headers
                        .lines()
                        .filter_map(|line| line.split_once(':'))
                        .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
                        .and_then(|(_, value)| value.trim().parse::<usize>().ok())
                        .unwrap_or_default();
                    if request.len() >= end + 4 + length {
                        break;
                    }
                }
            }
            let request = String::from_utf8(request).map_err(std::io::Error::other)?;
            let headers = request
                .lines()
                .filter_map(|line| line.split_once(':'))
                .map(|(name, value)| (name.to_ascii_lowercase(), value.trim().to_string()))
                .collect::<std::collections::HashMap<_, _>>();
            requests.push((
                headers.get("authorization").cloned(),
                headers.get("tracestate").cloned(),
            ));
            token_file.publish(b"B");
            let body = if index == 0 {
                "truncated"
            } else {
                r#"{"data":{"value":"accepted"}}"#
            };
            let length = if index == 0 { 10000 } else { body.len() };
            connection.write_all(format!(
                    "HTTP/1.1 {status} Test\r\ncontent-length: {length}\r\nconnection: close\r\n\r\n"
                ).as_bytes()).await?;
            if index == 0 && slow_body {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            // The client can legitimately close a timed-out body before it is written.
            let _ = connection.write_all(body.as_bytes()).await;
            connection.shutdown().await?;
        }
        Ok::<_, std::io::Error>(())
    };
    let client = token_file
        .client_at(&endpoint)
        .with_external_attempt_timeout(if slow_body {
            Duration::from_millis(50)
        } else {
            Duration::from_secs(2)
        });

    let result = tokio::time::timeout(Duration::from_secs(3), async {
        let operation = transport.execute(&client);
        tokio::pin!(operation);
        tokio::select! {
            result = &mut operation => result,
            result = peer => {
                assert_that!(result.map_err(|error| error.to_string()))
                    .is_ok()
                    .is_equal_to(());
                operation.await
            }
        }
    })
    .await
    .unwrap();
    let mut expected = vec![(
        Some("Bearer A".to_string()),
        Some("rover=retries:0".to_string()),
    )];
    if expects_second_attempt {
        expected.push((
            Some("Bearer B".to_string()),
            Some("rover=retries:1".to_string()),
        ));
        assert_that!(result)
            .is_ok()
            .is_equal_to(json!({"value": "accepted"}));
    } else if status == 503 {
        let error = &assert_that!(result).is_err().subject;
        let RoverClientError::SendRequest { source, .. } = error else {
            panic!("Expected the HTTP status error before reading its body, got {error:?}");
        };
        assert_that!(source.status())
            .is_some()
            .is_equal_to(http::StatusCode::SERVICE_UNAVAILABLE);
    } else {
        let error = &assert_that!(result).is_err().subject;
        let RoverClientError::Service { source, .. } = error else {
            panic!("Expected a response body failure, got {error:?}");
        };
        let graphql_error = source.downcast_ref::<GraphQLServiceError<()>>();
        let GraphQLServiceError::UpstreamService(source) =
            assert_that!(graphql_error).is_some().subject
        else {
            panic!("Expected an upstream HTTP service error, got {source:?}");
        };
        assert_that!(source.downcast_ref::<HttpServiceError>())
            .is_some()
            .matches(|error| {
                if slow_body {
                    matches!(error, HttpServiceError::TimedOut)
                } else {
                    matches!(error, HttpServiceError::Body(_))
                }
            });
        assert_that!(source.to_string()).is_equal_to(
            if slow_body {
                "Request timed out"
            } else {
                "Body error"
            }
            .to_string(),
        );
    }
    assert_that!(requests).is_equal_to(expected);
}

#![warn(missing_docs)]
#![cfg_attr(
    not(test),
    deny(
        clippy::exit,
        clippy::panic,
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::indexing_slicing,
    )
)]

//! Provides GraphQL Middleware for HTTP Services

use std::{convert::Infallible, fmt, future::Future, pin::Pin, str::FromStr};

use bytes::Bytes;
use graphql_client::GraphQLQuery;
use http::{uri::InvalidUri, HeaderValue, Method, StatusCode, Uri};
use http_body_util::Full;
use rover_http::{BodyExt, HttpRequest, HttpResponse};
use serde::{Deserialize, Serialize};
use tower::{Layer, Service};
use url::Url;

const JSON_CONTENT_TYPE: &str = "application/json";

#[derive(Serialize, Deserialize, Debug, Clone)]
struct PartialErrorInnerError {
    message: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct PartialErrorInnerErrorList {
    errors: Vec<PartialErrorInnerError>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct PartialErrorInnerBody {
    body: PartialErrorInnerErrorList,
}

#[derive(Debug, Clone)]
struct SimplifiedErrorList {
    errors: Vec<String>,
}

impl From<&Vec<graphql_client::Error>> for SimplifiedErrorList {
    fn from(errors: &Vec<graphql_client::Error>) -> Self {
        Self {
            errors: errors
                .iter()
                .flat_map(|error| {
                    // Some upstream services wrap a nested error body in
                    // `extensions.response` (e.g. a federated subgraph's own
                    // error response); surface those messages too. But
                    // always include the error's own top-level `message`
                    // first - that's where Studio's own credential-rejection
                    // errors ("Unauthorized: Invalid credentials provided")
                    // actually live, with no nested `response` extension at all.
                    let nested = error
                        .extensions
                        .as_ref()
                        .and_then(|extensions| extensions.get("response"))
                        .map(ToString::to_string)
                        .and_then(|response| {
                            serde_json::from_str::<PartialErrorInnerBody>(&response).ok()
                        })
                        .map(|partial_error_inner_body| partial_error_inner_body.body.errors)
                        .into_iter()
                        .flatten()
                        .map(|partial_error_inner_error| partial_error_inner_error.message);
                    std::iter::once(error.message.clone()).chain(nested)
                })
                .collect(),
        }
    }
}

/// Re-export / renamed type alias for [`graphql_client::Response`]
pub type GraphQLResponse<T> = graphql_client::Response<T>;

/// Errors that may occur from using a [`GraphQLService`]
#[derive(thiserror::Error, Debug)]
pub enum GraphQLServiceError<T: Send + Sync + fmt::Debug> {
    /// There was no data field provided in the response
    #[error("No data field provided")]
    NoData(Vec<graphql_client::Error>),
    /// The response returned some data, but there were errors
    #[error("Data was returned, but with errors: {}", friendly_errors_detail.join(" "))]
    PartialError {
        /// The partial data returned
        data: T,
        /// The GraphQL errors that were produced
        errors: Vec<graphql_client::Error>,
        /// display ready decoration of `errors`
        friendly_errors_detail: Vec<String>,
    },
    /// The request failed to present credentials that authorize for the current request.
    #[error(
        "Invalid credentials provided. See \"Authenticating with GraphOS\" [https://www.apollographql.com/docs/rover/configuring]."
    )]
    InvalidCredentials(),
    /// Data serialization error
    #[error("Serialization error")]
    Serialization(serde_json::Error),
    /// Data deserialization error
    #[error("Deserialization error")]
    Deserialization {
        /// The source error
        error: serde_json::Error,
        /// The data that was attempted to be deserialized
        data: Bytes,
        /// The [`StatusCode`] of the request
        status_code: StatusCode,
    },
    /// [`http`]-related error, probably from header-related tasks
    #[error("HTTP error: {:?}", .0)]
    Http(#[from] http::Error),
    /// Error that occurs from a failure to parse a [`Uri`] from a [`Url`]
    #[error("Unable to convert URL to URI.")]
    InvalidUri(#[from] InvalidUri),
    /// Errors that occur as a result of the underlying [`HttpService`] failing
    #[error("Upstream service error: {:?}", .0)]
    UpstreamService(#[from] Box<dyn std::error::Error + Send + Sync>),
    /// This shouldn't ever happen
    #[error(transparent)]
    Infallible(#[from] Infallible),
}

/// Wrapper around [`GraphQLQuery::Variables`]
/// This type requires something more concrete around it to be used appropriately
pub struct GraphQLRequest<Q: GraphQLQuery> {
    variables: Q::Variables,
}

impl<Q> fmt::Debug for GraphQLRequest<Q>
where
    Q: GraphQLQuery,
    Q::Variables: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(f, "{:?}", self.variables)
    }
}

impl<Q> PartialEq for GraphQLRequest<Q>
where
    Q: GraphQLQuery,
    Q::Variables: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.variables == other.variables
    }
}

impl<Q: GraphQLQuery> GraphQLRequest<Q> {
    /// Constructs a new [`GraphQLRequest`]
    pub const fn new(variables: Q::Variables) -> GraphQLRequest<Q> {
        GraphQLRequest { variables }
    }
    /// Consumes the [`GraphQLRequest`] and produces the inner [`GraphQLQuery::Variables`] object
    pub fn into_inner(self) -> Q::Variables {
        self.variables
    }
}

/// [`Layer`] that wraps a service with GraphQL middleware
#[derive(Default)]
pub struct GraphQLLayer {
    endpoint: Option<Url>,
}

impl GraphQLLayer {
    /// Constructs a new [`GraphQLLayer`]
    pub const fn new(endpoint: Url) -> GraphQLLayer {
        GraphQLLayer {
            endpoint: Some(endpoint),
        }
    }
}

impl<S> Layer<S> for GraphQLLayer {
    type Service = GraphQLService<S>;
    fn layer(&self, inner: S) -> Self::Service {
        GraphQLService::new(self.endpoint.clone(), inner)
    }
}

/// Middleware that wraps a service in GraphQL functionality
#[derive(Clone, Debug)]
pub struct GraphQLService<S> {
    inner: S,
    endpoint: Option<Url>,
}

impl<S> GraphQLService<S> {
    /// Constructs a new [`GraphQLService`]
    pub const fn new(endpoint: Option<Url>, inner: S) -> GraphQLService<S> {
        GraphQLService { endpoint, inner }
    }
}

impl<Q, S> Service<GraphQLRequest<Q>> for GraphQLService<S>
where
    Q: GraphQLQuery + Send + Sync + 'static,
    Q::Variables: Send,
    Q::ResponseData: Send + Sync + fmt::Debug,
    S: Service<HttpRequest, Response = HttpResponse> + Clone + Send + 'static,
    S::Future: Send,
    S::Error: std::error::Error + Send + Sync,
{
    type Response = Q::ResponseData;
    type Error = GraphQLServiceError<Q::ResponseData>;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        Service::poll_ready(&mut self.inner, cx)
            .map_err(|err| GraphQLServiceError::UpstreamService(Box::new(err)))
    }

    fn call(&mut self, req: GraphQLRequest<Q>) -> Self::Future {
        // https://docs.rs/tower/latest/tower/trait.Service.html#be-careful-when-cloning-inner-services
        let cloned = self.inner.clone();
        let mut client = std::mem::replace(&mut self.inner, cloned);

        let url = self.endpoint.clone();

        let fut = async move {
            let body = Q::build_query(req.into_inner());
            let body_bytes =
                Bytes::from(serde_json::to_vec(&body).map_err(GraphQLServiceError::Serialization)?);
            let req = http::Request::builder();
            let req = if let Some(url) = url.as_ref() {
                req.uri(Uri::from_str(url.as_ref())?)
            } else {
                req
            };
            let req = req
                .method(Method::POST)
                .header(
                    http::header::CONTENT_TYPE,
                    HeaderValue::from_static(JSON_CONTENT_TYPE),
                )
                .body(Full::new(body_bytes))
                .map_err(GraphQLServiceError::Http)?;
            let resp = client
                .call(req)
                .await
                .map_err(|err| GraphQLServiceError::UpstreamService(Box::new(err)))?;
            let status_code = resp.status();
            let body = resp.into_body().collect().await?.to_bytes();
            let graphql_response: graphql_client::Response<Q::ResponseData> =
                serde_json::from_slice(&body[..]).map_err(|err| {
                    GraphQLServiceError::Deserialization {
                        error: err,
                        data: body.clone(),
                        status_code,
                    }
                })?;

            if let Some(errors) = graphql_response.errors {
                let friendly_errors_detail = SimplifiedErrorList::from(&errors).errors;

                // Checked once, regardless of whether the response also carried a
                // `data` field - a credential rejection is a credential rejection
                // whether or not the server happened to include `"data": null` or
                // omit the field entirely.
                if friendly_errors_detail.iter().any(|message| {
                    let message = message.to_lowercase();
                    message.contains("unauthorized") || message.contains("invalid credentials")
                }) {
                    return Err(GraphQLServiceError::InvalidCredentials {});
                }

                match graphql_response.data {
                    Some(data) => Err(GraphQLServiceError::PartialError {
                        data,
                        errors,
                        friendly_errors_detail,
                    }),
                    None => Err(GraphQLServiceError::NoData(errors)),
                }
            } else {
                graphql_response
                    .data
                    .ok_or_else(|| GraphQLServiceError::NoData(Vec::default()))
            }
        };
        Box::pin(fut)
    }
}

//noinspection HttpUrlsUsage
#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use anyhow::Result;
    use bytes::Bytes;
    use futures::future;
    use graphql_client::{GraphQLQuery, QueryBody};
    use http::{HeaderValue, Method, StatusCode, Uri};
    use rover_http::{
        body::body_to_bytes, test::MockHttpService, Full, HttpRequest, HttpResponse,
        HttpServiceError,
    };
    use rover_tower::test::{expect_poll_ready, MockCloneService};
    use rstest::rstest;
    use serde::{Deserialize, Serialize};
    use speculoos::prelude::*;
    use tokio::task;
    use tower::{Service, ServiceBuilder, ServiceExt};
    use tower_test::mock;
    use url::Url;

    use super::{
        GraphQLLayer, GraphQLRequest, GraphQLService, GraphQLServiceError, JSON_CONTENT_TYPE,
    };

    struct TestQuery {}

    #[derive(Serialize)]
    struct TestQueryVariables {
        variable: i32,
    }

    #[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
    struct TestQueryResponse {
        inner_data: i32,
    }

    impl GraphQLQuery for TestQuery {
        type Variables = TestQueryVariables;
        type ResponseData = TestQueryResponse;

        fn build_query(variables: Self::Variables) -> QueryBody<Self::Variables> {
            QueryBody {
                variables,
                query: "query AskAQuestion { __typename }",
                operation_name: "AskAQuestion",
            }
        }
    }

    #[tokio::test]
    pub async fn test_successful_request() {
        let endpoint = Url::parse("http://example.com/graphql").unwrap();
        let (mock_service, mut handle) = mock::spawn::<HttpRequest, HttpResponse>();
        let mut service = ServiceBuilder::new()
            .layer(GraphQLLayer::new(endpoint.clone()))
            .map_err(HttpServiceError::Unexpected)
            .service(mock_service.into_inner());
        let service = ServiceExt::<GraphQLRequest<TestQuery>>::ready(&mut service)
            .await
            .unwrap();

        let variables = TestQueryVariables { variable: 7 };
        let request: GraphQLRequest<TestQuery> = GraphQLRequest::new(variables);
        let service_call_fut = service.call(request);

        task::spawn(async move {
            let (mut actual, send_response) = handle.next_request().await.unwrap();

            // Ensures that the request looks like we want it to
            assert_that!(actual.uri()).is_equal_to(&Uri::from_str(endpoint.as_str()).unwrap());
            assert_that!(actual.method()).is_equal_to(&Method::POST);
            assert_that!(actual.headers().get(http::header::CONTENT_TYPE).unwrap())
                .is_equal_to(&HeaderValue::from_static(JSON_CONTENT_TYPE));

            // Flattens out the bodies to bytes, as `Full<Bytes>` can't be evaluated
            let request_body = body_to_bytes(actual.body_mut()).await.unwrap();
            let expected_query_body = TestQuery::build_query(TestQueryVariables { variable: 7 });
            let expected_json_query_body =
                Bytes::from(serde_json::to_vec(&expected_query_body).unwrap());
            assert_that!(request_body).is_equal_to(expected_json_query_body);

            let graphql_response = graphql_client::Response {
                data: Some(TestQueryResponse { inner_data: 14 }),
                errors: None,
                extensions: None,
            };
            let mock_http_response = http::Response::builder()
                .body(Full::new(Bytes::from(
                    serde_json::to_vec(&graphql_response).unwrap(),
                )))
                .unwrap();
            send_response.send_response(mock_http_response);
        });

        let result = service_call_fut.await;

        assert_that!(result)
            .is_ok()
            .is_equal_to(TestQueryResponse { inner_data: 14 });
    }

    #[tokio::test]
    pub async fn test_error_no_data() -> Result<()> {
        let endpoint = Url::parse("http://example.com/graphql")?;
        let (mock_service, mut handle) = mock::spawn::<HttpRequest, HttpResponse>();
        let mut service = ServiceBuilder::new()
            .layer(GraphQLLayer::new(endpoint.clone()))
            .map_err(HttpServiceError::Unexpected)
            .service(mock_service.into_inner());
        let service = ServiceExt::<GraphQLRequest<TestQuery>>::ready(&mut service).await?;

        let variables = TestQueryVariables { variable: 7 };
        let request: GraphQLRequest<TestQuery> = GraphQLRequest::new(variables);
        let service_call_fut = service.call(request);

        // Background task that asserts conditions about the request to the mock service
        // and returns a mocked response
        task::spawn(async move {
            let (mut actual, send_response) = handle.next_request().await.unwrap();

            // Ensures that the request looks like we want it to
            assert_that!(actual.uri()).is_equal_to(&Uri::from_str(endpoint.as_str()).unwrap());
            assert_that!(actual.method()).is_equal_to(&Method::POST);
            assert_that!(actual.headers().get(http::header::CONTENT_TYPE).unwrap())
                .is_equal_to(&HeaderValue::from_static(JSON_CONTENT_TYPE));

            // Flattens out the bodies to bytes, as `Full<Bytes>` can't be evaluated
            let request_body = body_to_bytes(actual.body_mut()).await.unwrap();
            let expected_query_body = TestQuery::build_query(TestQueryVariables { variable: 7 });
            let expected_json_query_body =
                Bytes::from(serde_json::to_vec(&expected_query_body).unwrap());
            assert_that!(request_body).is_equal_to(expected_json_query_body);

            let error = graphql_client::Error {
                message: "something went wrong".to_string(),
                locations: None,
                path: None,
                extensions: None,
            };

            let graphql_response: graphql_client::Response<TestQueryResponse> =
                graphql_client::Response {
                    data: None,
                    errors: Some(vec![error]),
                    extensions: None,
                };
            let mock_http_response = http::Response::builder()
                .body(Full::new(Bytes::from(
                    serde_json::to_vec(&graphql_response).unwrap(),
                )))
                .unwrap();
            send_response.send_response(mock_http_response);
        });

        let result = service_call_fut.await;

        assert_that!(result).is_err().matches(|err| match err {
            GraphQLServiceError::NoData(errors) => {
                errors
                    == &vec![graphql_client::Error {
                        message: "something went wrong".to_string(),
                        locations: None,
                        path: None,
                        extensions: None,
                    }]
            }
            _ => false,
        });
        Ok(())
    }

    // Studio's own credential-rejection response has `"data": null` and puts
    // the credential-rejection message directly on the error, not nested
    // under `extensions.response`. The check must catch it whether the
    // message mentions "unauthorized", "invalid credentials", both, or
    // either in a different case.
    #[rstest]
    #[case::unauthorized_only("Unauthorized")]
    #[case::invalid_credentials_only("Invalid credentials provided")]
    #[case::both("Unauthorized: Invalid credentials provided")]
    #[case::mixed_case("UNAUTHORIZED: INVALID CREDENTIALS PROVIDED")]
    #[tokio::test]
    pub async fn test_invalid_credentials_with_null_data(#[case] message: &str) -> Result<()> {
        let message = message.to_string();
        let mut mock = MockHttpService::new();
        expect_poll_ready!(mock);
        mock.expect_call().returning(move |_| {
            let error = graphql_client::Error {
                message: message.clone(),
                locations: None,
                path: None,
                extensions: None,
            };
            let graphql_response: graphql_client::Response<TestQueryResponse> =
                graphql_client::Response {
                    data: None,
                    errors: Some(vec![error]),
                    extensions: None,
                };
            let mock_http_response = http::Response::builder()
                .body(Full::new(Bytes::from(
                    serde_json::to_vec(&graphql_response).unwrap(),
                )))
                .unwrap();
            future::ready(Ok(mock_http_response))
        });

        let service = GraphQLService::new(None, MockCloneService::new(mock));
        let request: GraphQLRequest<TestQuery> =
            GraphQLRequest::new(TestQueryVariables { variable: 7 });
        let result = service.oneshot(request).await;

        assert_that!(result)
            .is_err()
            .matches(|err| matches!(err, GraphQLServiceError::InvalidCredentials()));
        Ok(())
    }

    // Same credential-rejection messages, but the response also carries a
    // (partial) `data` field - confirms the check isn't skipped just because
    // `data` happened to be present.
    #[rstest]
    #[case::unauthorized_only("Unauthorized")]
    #[case::invalid_credentials_only("Invalid credentials provided")]
    #[case::both("Unauthorized: Invalid credentials provided")]
    #[case::mixed_case("UNAUTHORIZED: INVALID CREDENTIALS PROVIDED")]
    #[tokio::test]
    pub async fn test_invalid_credentials_with_partial_data(#[case] message: &str) -> Result<()> {
        let message = message.to_string();
        let mut mock = MockHttpService::new();
        expect_poll_ready!(mock);
        mock.expect_call().returning(move |_| {
            let error = graphql_client::Error {
                message: message.clone(),
                locations: None,
                path: None,
                extensions: None,
            };
            let graphql_response: graphql_client::Response<TestQueryResponse> =
                graphql_client::Response {
                    data: Some(TestQueryResponse { inner_data: 0 }),
                    errors: Some(vec![error]),
                    extensions: None,
                };
            let mock_http_response = http::Response::builder()
                .body(Full::new(Bytes::from(
                    serde_json::to_vec(&graphql_response).unwrap(),
                )))
                .unwrap();
            future::ready(Ok(mock_http_response))
        });

        let service = GraphQLService::new(None, MockCloneService::new(mock));
        let request: GraphQLRequest<TestQuery> =
            GraphQLRequest::new(TestQueryVariables { variable: 7 });
        let result = service.oneshot(request).await;

        assert_that!(result)
            .is_err()
            .matches(|err| matches!(err, GraphQLServiceError::InvalidCredentials()));
        Ok(())
    }

    // A non-credential error with `data` present must still fall through to
    // `PartialError`, not `InvalidCredentials` - the broadened substring
    // check must not swallow unrelated errors.
    #[tokio::test]
    pub async fn test_partial_error_when_not_a_credential_rejection() -> Result<()> {
        let mut mock = MockHttpService::new();
        expect_poll_ready!(mock);
        mock.expect_call().returning(|_| {
            let error = graphql_client::Error {
                message: "something went wrong".to_string(),
                locations: None,
                path: None,
                extensions: None,
            };
            let graphql_response: graphql_client::Response<TestQueryResponse> =
                graphql_client::Response {
                    data: Some(TestQueryResponse { inner_data: 0 }),
                    errors: Some(vec![error]),
                    extensions: None,
                };
            let mock_http_response = http::Response::builder()
                .body(Full::new(Bytes::from(
                    serde_json::to_vec(&graphql_response).unwrap(),
                )))
                .unwrap();
            future::ready(Ok(mock_http_response))
        });

        let service = GraphQLService::new(None, MockCloneService::new(mock));
        let request: GraphQLRequest<TestQuery> =
            GraphQLRequest::new(TestQueryVariables { variable: 7 });
        let result = service.oneshot(request).await;

        assert_that!(result)
            .is_err()
            .matches(|err| matches!(err, GraphQLServiceError::PartialError { .. }));
        Ok(())
    }

    #[rstest]
    #[case::ok(StatusCode::OK)]
    #[case::internal_server_error(StatusCode::INTERNAL_SERVER_ERROR)]
    #[tokio::test]
    pub async fn test_json_deserialization_error(#[case] expected_status_code: StatusCode) {
        let endpoint = Url::parse("http://example.com/graphql").unwrap();
        let (mock_service, mut handle) = mock::spawn::<HttpRequest, HttpResponse>();
        let mut service = ServiceBuilder::new()
            .layer(GraphQLLayer::new(endpoint.clone()))
            .map_err(HttpServiceError::Unexpected)
            .service(mock_service.into_inner());
        let service = ServiceExt::<GraphQLRequest<TestQuery>>::ready(&mut service)
            .await
            .unwrap();

        let variables = TestQueryVariables { variable: 7 };
        let request: GraphQLRequest<TestQuery> = GraphQLRequest::new(variables);
        let service_call_fut = service.call(request);

        task::spawn(async move {
            let (mut actual, send_response) = handle.next_request().await.unwrap();

            // Ensures that the request looks like we want it to
            assert_that!(actual.uri()).is_equal_to(&Uri::from_str(endpoint.as_str()).unwrap());
            assert_that!(actual.method()).is_equal_to(&Method::POST);
            assert_that!(actual.headers().get(http::header::CONTENT_TYPE).unwrap())
                .is_equal_to(&HeaderValue::from_static(JSON_CONTENT_TYPE));

            // Flattens out the bodies to bytes, as `Full<Bytes>` can't be evaluated
            let request_body = body_to_bytes(actual.body_mut()).await.unwrap();
            let expected_query_body = TestQuery::build_query(TestQueryVariables { variable: 7 });
            let expected_json_query_body =
                Bytes::from(serde_json::to_vec(&expected_query_body).unwrap());
            assert_that!(request_body).is_equal_to(expected_json_query_body);

            let response = "something went wrong";
            let mock_http_response = http::Response::builder()
                .status(expected_status_code)
                .body(Full::new(Bytes::from(response.as_bytes())))
                .unwrap();
            send_response.send_response(mock_http_response);
        });

        let result = service_call_fut.await;

        assert_that!(result).is_err().matches(|err| match err {
            GraphQLServiceError::Deserialization {
                data, status_code, ..
            } => {
                status_code == &expected_status_code
                    && data == &Bytes::from("something went wrong".as_bytes())
            }
            _ => false,
        });
    }
}

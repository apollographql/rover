#![warn(missing_docs)]

//! Provides GraphQL Middleware for HTTP Services

use std::{fmt::Debug, future::Future, pin::Pin, str::FromStr};

use bytes::Bytes;
use graphql_client::GraphQLQuery;
use http::{uri::InvalidUri, HeaderValue, Method, StatusCode, Uri};
use http_body_util::Full;
use rover_http::{HttpRequest, HttpResponse, HttpServiceError};
use tower::{Layer, Service};
use url::Url;

const JSON_CONTENT_TYPE: &str = "application/json";

/// Re-export / renamed type alias for [`graphql_client::Response`]
pub type GraphQLResponse<T> = graphql_client::Response<T>;

/// Errors that may occur from using a [`GraphQLService`]
#[derive(thiserror::Error, Debug)]
pub enum GraphQLServiceError<T: Send + Sync + Debug> {
    /// There was not data field provided in the response
    #[error("No data field provided")]
    NoData(Vec<graphql_client::Error>),
    /// The response returned some data, but there were errors
    #[error("Data was returned, but with errors")]
    PartialError {
        /// The partial data returned
        data: T,
        /// The GraphQL errors that were produced
        errors: Vec<graphql_client::Error>,
    },
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
    UpstreamService(#[from] HttpServiceError),
}

/// Wrapper around [`GraphQLQuery::Variables`]
/// This type requires something more concrete around it to be used appropriately
pub struct GraphQLRequest<Q: GraphQLQuery> {
    variables: Q::Variables,
}

impl<Q: GraphQLQuery> GraphQLRequest<Q> {
    /// Constructs a new [`GraphQLRequest`]
    pub fn new(variables: Q::Variables) -> GraphQLRequest<Q> {
        GraphQLRequest { variables }
    }
    /// Consumes the [`GraphQLRequest`] and produces the inner [`GraphQLQuery::Variables`] object
    pub fn into_inner(self) -> Q::Variables {
        self.variables
    }
}

/// [`Layer`] that wraps a service with GraphQL middleware
pub struct GraphQLLayer {
    endpoint: Url,
}

impl GraphQLLayer {
    /// Constructs a new [`GraphQLLayer`]
    pub fn new(endpoint: Url) -> GraphQLLayer {
        GraphQLLayer { endpoint }
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
    endpoint: Url,
}

impl<S> GraphQLService<S> {
    /// Constructs a new [`GraphQLService`]
    pub fn new(endpoint: Url, inner: S) -> GraphQLService<S> {
        GraphQLService { endpoint, inner }
    }
}

impl<Q, S> Service<GraphQLRequest<Q>> for GraphQLService<S>
where
    Q: GraphQLQuery + Send + Sync + 'static,
    Q::Variables: Send,
    Q::ResponseData: Send + Sync + Debug,
    S: Service<HttpRequest, Response = HttpResponse, Error = HttpServiceError>
        + Clone
        + Send
        + 'static,
    S::Future: Send,
{
    type Response = Q::ResponseData;
    type Error = GraphQLServiceError<Q::ResponseData>;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        tower::Service::poll_ready(&mut self.inner, cx).map_err(GraphQLServiceError::from)
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
            let req = http::Request::builder()
                .uri(Uri::from_str(url.as_ref())?)
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
                .map_err(GraphQLServiceError::UpstreamService)?;
            let body = resp.body();
            let graphql_response: graphql_client::Response<Q::ResponseData> =
                serde_json::from_slice(body).map_err(|err| {
                    GraphQLServiceError::Deserialization {
                        error: err,
                        data: body.clone(),
                        status_code: resp.status(),
                    }
                })?;
            if let Some(errors) = graphql_response.errors {
                match graphql_response.data {
                    Some(data) => Err(GraphQLServiceError::PartialError { data, errors }),
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

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use anyhow::Result;
    use bytes::Bytes;
    use graphql_client::{GraphQLQuery, QueryBody};
    use http::{HeaderValue, Method, StatusCode, Uri};
    use rover_http::{body::body_to_bytes, HttpServiceError};
    use rstest::rstest;
    use serde::{Deserialize, Serialize};
    use speculoos::prelude::*;
    use tokio::task;
    use tokio_test::{assert_ready, assert_ready_ok};
    use tower::ServiceBuilder;
    use tower_test::mock;
    use url::Url;

    use crate::{GraphQLLayer, GraphQLRequest, GraphQLServiceError, JSON_CONTENT_TYPE};

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

        fn build_query(variables: Self::Variables) -> graphql_client::QueryBody<Self::Variables> {
            QueryBody {
                variables,
                query: "query AskAQuestion { __typename }",
                operation_name: "AskAQuestion",
            }
        }
    }

    #[tokio::test]
    pub async fn test_successful_request() -> Result<()> {
        let endpoint = Url::parse("http://example.com/graphql")?;
        let (mut service, mut handle) = mock::spawn_with(|inner| {
            ServiceBuilder::new()
                .layer(GraphQLLayer::new(endpoint.clone()))
                .map_err(HttpServiceError::Unexpected)
                .service(inner)
        });
        assert_ready_ok!(service.poll_ready::<GraphQLRequest<TestQuery>>());

        let variables = TestQueryVariables { variable: 7 };
        let request: GraphQLRequest<TestQuery> = GraphQLRequest::new(variables);
        let service_call_fut = task::spawn(service.call(request));

        // Background task that asserts conditions about the request to the mock service
        // and returns a mocked response
        task::spawn(async move {
            let (mut actual, send_response) = match handle.next_request().await {
                Some(r) => r,
                None => panic!("expected a request but none was received."),
            };

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
                .body(Bytes::from(serde_json::to_vec(&graphql_response).unwrap()))
                .unwrap();
            send_response.send_response(mock_http_response);
        });

        let result = service_call_fut.await?;

        assert_that!(result)
            .is_ok()
            .is_equal_to(TestQueryResponse { inner_data: 14 });
        Ok(())
    }

    #[tokio::test]
    pub async fn test_error_no_data() -> Result<()> {
        let endpoint = Url::parse("http://example.com/graphql")?;
        let (mut service, mut handle) = mock::spawn_with(|inner| {
            ServiceBuilder::new()
                .layer(GraphQLLayer::new(endpoint.clone()))
                .map_err(HttpServiceError::Unexpected)
                .service(inner)
        });
        assert_ready_ok!(service.poll_ready::<GraphQLRequest<TestQuery>>());

        let variables = TestQueryVariables { variable: 7 };
        let request: GraphQLRequest<TestQuery> = GraphQLRequest::new(variables);
        let service_call_fut = task::spawn(service.call(request));

        // Background task that asserts conditions about the request to the mock service
        // and returns a mocked response
        task::spawn(async move {
            let (mut actual, send_response) = match handle.next_request().await {
                Some(r) => r,
                None => panic!("expected a request but none was received."),
            };

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
                .body(Bytes::from(serde_json::to_vec(&graphql_response).unwrap()))
                .unwrap();
            send_response.send_response(mock_http_response);
        });

        let result = service_call_fut.await?;

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

    #[rstest]
    #[case::ok(StatusCode::OK)]
    #[case::internal_server_error(StatusCode::INTERNAL_SERVER_ERROR)]
    #[tokio::test]
    pub async fn test_json_deserialization_error(
        #[case] expected_status_code: StatusCode,
    ) -> Result<()> {
        let endpoint = Url::parse("http://example.com/graphql")?;
        let (mut service, mut handle) = mock::spawn_with(|inner| {
            ServiceBuilder::new()
                .layer(GraphQLLayer::new(endpoint.clone()))
                .map_err(HttpServiceError::Unexpected)
                .service(inner)
        });
        assert_ready_ok!(service.poll_ready::<GraphQLRequest<TestQuery>>());

        let variables = TestQueryVariables { variable: 7 };
        let request: GraphQLRequest<TestQuery> = GraphQLRequest::new(variables);
        let service_call_fut = task::spawn(service.call(request));

        // Background task that asserts conditions about the request to the mock service
        // and returns a mocked response
        task::spawn(async move {
            let (mut actual, send_response) = match handle.next_request().await {
                Some(r) => r,
                None => panic!("expected a request but none was received."),
            };

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
                .body(Bytes::from(response.as_bytes()))
                .unwrap();
            send_response.send_response(mock_http_response);
        });

        let result = service_call_fut.await?;

        assert_that!(result).is_err().matches(|err| match err {
            GraphQLServiceError::Deserialization {
                data, status_code, ..
            } => {
                status_code == &expected_status_code
                    && data == &Bytes::from("something went wrong".as_bytes())
            }
            _ => false,
        });
        Ok(())
    }
}

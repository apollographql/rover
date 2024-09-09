use std::{fmt::Debug, future::Future, pin::Pin, str::FromStr, task::Poll};

use bytes::Bytes;
use graphql_client::GraphQLQuery;
use http::{uri::InvalidUri, HeaderValue, Uri};
use http_body_util::Full;
use rover_http::HttpServiceError;
use tower::{Layer, Service};
use url::Url;

const JSON_CONTENT_TYPE: &str = "application/json";

pub type GraphQLResponse<T> = graphql_client::Response<T>;

#[derive(thiserror::Error, Debug)]
pub enum GraphQLServiceError<T: Send + Sync + Debug> {
    #[error("No data field provided")]
    NoData(Vec<graphql_client::Error>),
    #[error("Data was returned, but with errors")]
    PartialError {
        data: T,
        errors: Vec<graphql_client::Error>,
    },
    #[error("JSON (de)serialization error: {:?}", .0)]
    Json(#[from] serde_json::Error),
    #[error("HTTP error: {:?}", .0)]
    Http(#[from] http::Error),
    #[error("Unable to convert URL to URI.")]
    InvalidUri(#[from] InvalidUri),
    #[error("Upstream service error: {:?}", .0)]
    UpstreamService(#[from] HttpServiceError),
}

pub struct GraphQLRequest<Q: GraphQLQuery> {
    variables: Q::Variables,
}

impl<Q: GraphQLQuery> GraphQLRequest<Q> {
    pub fn new(variables: Q::Variables) -> GraphQLRequest<Q> {
        GraphQLRequest { variables }
    }
    pub fn into_inner(self) -> Q::Variables {
        self.variables
    }
}

pub struct GraphQLLayer {
    endpoint: Url,
}

impl GraphQLLayer {
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

#[derive(Clone, Debug)]
pub struct GraphQLService<S> {
    inner: S,
    endpoint: Url,
}

impl<S> GraphQLService<S> {
    pub fn new(endpoint: Url, inner: S) -> GraphQLService<S> {
        GraphQLService { endpoint, inner }
    }
}

impl<Q, S> Service<GraphQLRequest<Q>> for GraphQLService<S>
where
    Q: GraphQLQuery + 'static,
    Q::Variables: Send,
    Q::ResponseData: Send + Sync + Debug,
    S: Service<
            http::Request<Full<Bytes>>,
            Response = http::Response<Bytes>,
            Error = HttpServiceError,
        > + Clone
        + Send
        + 'static,
    S::Future: Send,
{
    type Response = Q::ResponseData;
    type Error = GraphQLServiceError<Q::ResponseData>;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(
        &mut self,
        _: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: GraphQLRequest<Q>) -> Self::Future {
        let mut client = self.inner.clone();
        let url = self.endpoint.clone();

        let fut = async move {
            let body = Q::build_query(req.into_inner());
            let body_bytes =
                Bytes::from(serde_json::to_vec(&body).map_err(GraphQLServiceError::Json)?);
            let req = http::Request::builder()
                .uri(Uri::from_str(&url.to_string())?)
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
                serde_json::from_slice(body).map_err(GraphQLServiceError::Json)?;
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

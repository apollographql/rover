use std::{collections::HashMap, future::Future, pin::Pin, time::Duration};

use buildstructor::buildstructor;
use graphql_client::GraphQLQuery;
use http::{
    header::{InvalidHeaderName, InvalidHeaderValue},
    HeaderMap, HeaderName, HeaderValue,
};
use rover_graphql::{GraphQLLayer, GraphQLRequest, GraphQLService, GraphQLServiceError};
use rover_http::{extend_headers::ExtendHeadersLayer, retry::RetryPolicy, HttpService};
use tower::{retry::RetryLayer, Layer, Service, ServiceBuilder};

use crate::{EndpointKind, RoverClientError};

use super::SubgraphIntrospectResponse;

#[derive(GraphQLQuery)]
#[graphql(
    query_path = "src/operations/subgraph/introspect/introspect_query.graphql",
    schema_path = "src/operations/subgraph/introspect/introspect_schema.graphql",
    response_derives = "Eq, PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
pub struct SubgraphIntrospectQuery;

#[derive(thiserror::Error, Debug)]
pub enum SubgraphIntrospectError {
    #[error("Inner service failed to become ready.\n{}", .0)]
    ServiceReady(Box<dyn std::error::Error + Send + Sync>),
    #[error(transparent)]
    Service(Box<dyn std::error::Error + Send + Sync>),
    #[error("No introspection response available.")]
    NoResponse,
    #[error("This endpoint doesn't support subgraph introspection via the Query._service field")]
    SubgraphIntrospectionNotAvailable,
}

impl From<SubgraphIntrospectError> for RoverClientError {
    fn from(value: SubgraphIntrospectError) -> Self {
        match value {
            SubgraphIntrospectError::ServiceReady(err) => RoverClientError::ServiceReady(err),
            SubgraphIntrospectError::Service(err) => RoverClientError::Service {
                source: err,
                endpoint_kind: EndpointKind::Customer,
            },
            SubgraphIntrospectError::NoResponse => RoverClientError::IntrospectionError {
                msg: value.to_string(),
            },
            SubgraphIntrospectError::SubgraphIntrospectionNotAvailable => {
                RoverClientError::SubgraphIntrospectionNotAvailable
            }
        }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum SubgraphIntrospectLayerError {
    #[error(transparent)]
    HeaderName(#[from] InvalidHeaderName),
    #[error(transparent)]
    HeaderValue(#[from] InvalidHeaderValue),
}

impl From<SubgraphIntrospectLayerError> for RoverClientError {
    fn from(value: SubgraphIntrospectLayerError) -> Self {
        match value {
            SubgraphIntrospectLayerError::HeaderName(err) => RoverClientError::from(err),
            SubgraphIntrospectLayerError::HeaderValue(err) => RoverClientError::from(err),
        }
    }
}

pub struct SubgraphIntrospectLayer {
    endpoint: url::Url,
    headers: HeaderMap,
    should_retry: bool,
    retry_period: Duration,
}

#[buildstructor]
impl SubgraphIntrospectLayer {
    #[builder]
    pub fn new(
        endpoint: url::Url,
        headers: HashMap<String, String>,
        should_retry: bool,
        retry_period: Duration,
    ) -> Result<SubgraphIntrospectLayer, SubgraphIntrospectLayerError> {
        let mut header_map = HeaderMap::new();
        for (header_key, header_value) in headers {
            header_map.insert(
                HeaderName::from_bytes(header_key.as_bytes())?,
                HeaderValue::from_str(&header_value)?,
            );
        }
        Ok(SubgraphIntrospectLayer {
            endpoint,
            headers: header_map,
            should_retry,
            retry_period,
        })
    }
}

impl Layer<HttpService> for SubgraphIntrospectLayer {
    type Service = SubgraphIntrospect<GraphQLService<HttpService>>;
    fn layer(&self, inner: HttpService) -> Self::Service {
        let retry_layer = if self.should_retry {
            Some(RetryLayer::new(RetryPolicy::new(self.retry_period)))
        } else {
            None
        };
        let http_service_stack = ServiceBuilder::new()
            .boxed_clone()
            .option_layer(retry_layer)
            .layer(ExtendHeadersLayer::new(self.headers.clone()))
            .service(inner);
        let graphql_service_stack = ServiceBuilder::new()
            .layer(GraphQLLayer::new(self.endpoint.clone()))
            .service(http_service_stack);
        SubgraphIntrospect {
            inner: graphql_service_stack,
        }
    }
}

#[derive(Clone)]
pub struct SubgraphIntrospect<S: Clone> {
    inner: S,
}

impl<S, Fut> Service<()> for SubgraphIntrospect<S>
where
    S: Service<
            GraphQLRequest<SubgraphIntrospectQuery>,
            Response = subgraph_introspect_query::ResponseData,
            Error = GraphQLServiceError<subgraph_introspect_query::ResponseData>,
            Future = Fut,
        > + Clone
        + Send
        + 'static,
    Fut: Future<Output = Result<S::Response, S::Error>> + Send,
{
    type Response = SubgraphIntrospectResponse;
    type Error = SubgraphIntrospectError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        tower::Service::<GraphQLRequest<SubgraphIntrospectQuery>>::poll_ready(&mut self.inner, cx)
            .map_err(|err| SubgraphIntrospectError::ServiceReady(Box::new(err)))
    }

    fn call(&mut self, _req: ()) -> Self::Future {
        let cloned = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, cloned);
        let fut = async move {
            let response_data = inner
                .call(GraphQLRequest::<SubgraphIntrospectQuery>::new(
                    subgraph_introspect_query::Variables {},
                ))
                .await
                .map_err(|err| SubgraphIntrospectError::Service(Box::new(err)));
            match response_data {
                Ok(data) => {
                    let graph = data.service.ok_or(SubgraphIntrospectError::NoResponse)?;
                    Ok(SubgraphIntrospectResponse { result: graph.sdl })
                }
                Err(err) => {
                    // this is almost definitely a result of a graph not
                    // being federated, or not matching the federation spec
                    if err.to_string().contains("Cannot query field") {
                        Err(SubgraphIntrospectError::SubgraphIntrospectionNotAvailable)
                    } else {
                        Err(err)
                    }
                }
            }
        };
        Box::pin(fut)
    }
}

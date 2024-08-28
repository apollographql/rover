use std::{fmt::Debug, pin::Pin, str::FromStr, time::Duration};

use buildstructor::buildstructor;
use derive_getters::Getters;
use futures::{Future, FutureExt};
use graphql_client::GraphQLQuery;
use http::{
    header::{InvalidHeaderName, InvalidHeaderValue},
    HeaderMap, HeaderName, HeaderValue,
};
use rover_graphql::{GraphQLLayer, GraphQLRequest};
use rover_http::{extend_headers::ExtendHeadersLayer, retry::BackoffLayer, HttpService};
use tower::{util::BoxCloneService, Service, ServiceBuilder, ServiceExt};
use url::Url;

use crate::{EndpointKind, RoverClientError};

use super::GraphQLErrorsLayer;

pub trait IntrospectionQuery: GraphQLQuery {
    type Response;
    fn variables() -> Self::Variables;
    fn map_response(response_data: Self::ResponseData) -> Result<Self::Response, RoverClientError>;
}

#[derive(Clone, Debug)]
pub enum RetryConfig {
    NoRetry,
    RetryWithDefault,
    RetryWith(Duration),
}

impl RetryConfig {
    fn backoff_layer(&self) -> Option<BackoffLayer> {
        match self {
            RetryConfig::NoRetry => None,
            RetryConfig::RetryWithDefault => Some(BackoffLayer::new(Duration::from_secs(90))),
            RetryConfig::RetryWith(duration) => Some(BackoffLayer::new(*duration)),
        }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum IntrospectionConfigError {
    #[error(transparent)]
    HeaderName(#[from] InvalidHeaderName),
    #[error(transparent)]
    HeaderValue(#[from] InvalidHeaderValue),
}

#[derive(Clone, Debug, Getters)]
pub struct IntrospectionConfig {
    endpoint: Url,
    headers: http::HeaderMap,
    retry_config: RetryConfig,
}

#[buildstructor]
impl IntrospectionConfig {
    #[builder]
    pub fn new(
        endpoint: Url,
        headers: Option<Vec<(String, String)>>,
        should_retry: Option<bool>,
        retry_period: Option<Duration>,
    ) -> Result<IntrospectionConfig, IntrospectionConfigError> {
        let should_retry = should_retry.unwrap_or_default();
        let retry_config = if should_retry {
            match retry_period {
                Some(duration) => RetryConfig::RetryWith(duration),
                None => RetryConfig::RetryWithDefault,
            }
        } else {
            RetryConfig::NoRetry
        };
        let mut header_map = HeaderMap::new();
        if let Some(headers) = headers {
            for (name, value) in headers.iter() {
                let name = HeaderName::from_str(name)?;
                let value = HeaderValue::from_str(value)?;
                header_map.insert(name, value);
            }
        }
        Ok(IntrospectionConfig {
            endpoint,
            headers: header_map,
            retry_config,
        })
    }
}

pub struct IntrospectionService<Q: IntrospectionQuery> {
    inner: BoxCloneService<GraphQLRequest<Q>, <Q as GraphQLQuery>::ResponseData, RoverClientError>,
}

impl<Q> IntrospectionService<Q>
where
    Q: IntrospectionQuery + Send + Sync + 'static,
    Q::Variables: Send,
    Q::ResponseData: Debug + Send + Sync,
{
    pub fn new(config: IntrospectionConfig, http_service: HttpService) -> IntrospectionService<Q> {
        let inner = ServiceBuilder::new()
            .layer(GraphQLErrorsLayer::new(EndpointKind::Customer))
            .layer(GraphQLLayer::new(config.endpoint))
            .layer(ExtendHeadersLayer::new(config.headers.clone()))
            .option_layer(config.retry_config.backoff_layer())
            .service(http_service)
            .boxed_clone();
        IntrospectionService { inner }
    }
}

impl<Q: IntrospectionQuery> Service<()> for IntrospectionService<Q>
where
    Q::ResponseData: 'static,
{
    type Response = Q::Response;
    type Error = RoverClientError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        tower::Service::<GraphQLRequest<Q>>::poll_ready(&mut self.inner, cx).map_err(|err| {
            RoverClientError::GraphQl {
                msg: format!("{:?}", err),
            }
        })
    }

    fn call(&mut self, _: ()) -> Self::Future {
        let req = GraphQLRequest::new(Q::variables());
        self.inner
            .call(req)
            .map(|result| match result {
                Ok(data) => Q::map_response(data),
                Err(err) => {
                    if err.to_string().contains("Cannot query field") {
                        Err(RoverClientError::SubgraphIntrospectionNotAvailable)
                    } else {
                        Err(RoverClientError::GraphQl {
                            msg: format!("{:?}", err),
                        })
                    }
                }
            })
            .boxed()
    }
}

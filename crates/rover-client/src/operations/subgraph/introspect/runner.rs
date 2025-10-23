use http::{HeaderMap, HeaderName, HeaderValue};
use reqwest::Client;
use rover_graphql::GraphQLLayer;
use rover_http::{extend_headers::ExtendHeadersLayer, retry::RetryPolicy, ReqwestService};
use tower::{retry::RetryLayer, Service, ServiceBuilder, ServiceExt};

use super::SubgraphIntrospect;
use crate::{operations::subgraph::introspect::types::*, RoverClientError};

pub async fn run(
    input: SubgraphIntrospectInput,
    client: &Client,
) -> Result<SubgraphIntrospectResponse, RoverClientError> {
    let retry_layer = if input.should_retry {
        Some(RetryLayer::new(RetryPolicy::new(input.retry_period)))
    } else {
        None
    };

    let http_service = ReqwestService::builder()
        .client(client.clone())
        .build()
        .map_err(|err| RoverClientError::ServiceReady(Box::new(err)))?;

    let mut header_map = HeaderMap::new();

    for (header_key, header_value) in input.headers {
        header_map.insert(
            HeaderName::from_bytes(header_key.as_bytes())?,
            HeaderValue::from_str(&header_value)?,
        );
    }

    let http_service_stack = ServiceBuilder::new()
        .boxed_clone()
        .option_layer(retry_layer)
        .layer(ExtendHeadersLayer::new(header_map))
        .service(http_service);

    let mut service = ServiceBuilder::new()
        .layer_fn(SubgraphIntrospect::new)
        .layer(GraphQLLayer::new(input.endpoint.clone()))
        .service(http_service_stack);

    let service = service.ready().await?;
    let resp = service.call(()).await?;
    Ok(resp)
}

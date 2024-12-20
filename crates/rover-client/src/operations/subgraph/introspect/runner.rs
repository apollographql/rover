use reqwest::Client;
use rover_http::ReqwestService;
use tower::{Service, ServiceBuilder, ServiceExt};

use crate::operations::subgraph::introspect::types::*;
use crate::RoverClientError;

use super::service::SubgraphIntrospectLayer;

pub async fn run(
    input: SubgraphIntrospectInput,
    client: &Client,
) -> Result<SubgraphIntrospectResponse, RoverClientError> {
    let subgraph_introspect_layer = SubgraphIntrospectLayer::builder()
        .endpoint(input.endpoint)
        .headers(input.headers)
        .should_retry(input.should_retry)
        .retry_period(input.retry_period)
        .build()?;
    let mut service = ServiceBuilder::new()
        .layer(subgraph_introspect_layer)
        .service(
            ReqwestService::builder()
                .client(client.clone())
                .build()
                .map_err(|err| RoverClientError::ServiceReady(Box::new(err)))?
                .boxed_clone(),
        );
    let service = service.ready().await?;
    let resp = service.call(()).await?;
    Ok(resp)
}

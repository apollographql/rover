use tower::{Service, ServiceExt};

use crate::blocking::StudioClient;
use crate::shared::FetchResponse;
use crate::RoverClientError;

use super::service::{SubgraphFetch, SubgraphFetchRequest};
use super::types::*;

/// Fetches a schema from apollo studio and returns its SDL (String)
pub async fn run(
    input: SubgraphFetchInput,
    client: &StudioClient,
) -> Result<FetchResponse, RoverClientError> {
    let mut service = SubgraphFetch::new(
        client
            .studio_graphql_service()
            .map_err(|err| RoverClientError::ServiceReady(Box::new(err)))?,
    );
    let service = service.ready().await?;
    let fetch_response = service.call(SubgraphFetchRequest::from(input)).await?;
    Ok(fetch_response)
}

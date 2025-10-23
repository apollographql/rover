use tower::{Service, ServiceExt};

use super::{
    service::{SubgraphFetch, SubgraphFetchRequest},
    types::*,
};
use crate::{blocking::StudioClient, shared::FetchResponse, RoverClientError};

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

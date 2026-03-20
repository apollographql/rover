use tower::{Service, ServiceExt};

use super::{
    service::{SupergraphFetch, SupergraphFetchRequest},
    types::*,
};
use crate::{blocking::StudioClient, shared::FetchResponse, RoverClientError};

/// Fetches a core schema from apollo studio
pub async fn run(
    input: SupergraphFetchInput,
    client: &StudioClient,
) -> Result<FetchResponse, RoverClientError> {
    let mut service = SupergraphFetch::new(
        client
            .studio_graphql_service()
            .map_err(|err| RoverClientError::ServiceReady(Box::new(err)))?,
    );
    let service = service.ready().await?;
    let fetch_response = service.call(SupergraphFetchRequest::from(input)).await?;
    Ok(fetch_response)
}

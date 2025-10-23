use tower::{Service, ServiceExt};

use super::{
    service::{SubgraphFetchAll, SubgraphFetchAllRequest},
    types::*,
};
use crate::{blocking::StudioClient, RoverClientError};

/// For a given graph return all of its subgraphs as a list
pub async fn run(
    input: SubgraphFetchAllInput,
    client: &StudioClient,
) -> Result<SubgraphFetchAllResponse, RoverClientError> {
    let mut service = SubgraphFetchAll::new(
        client
            .studio_graphql_service()
            .map_err(|err| RoverClientError::ServiceReady(Box::new(err)))?,
    );
    let service = service.ready().await?;
    let subgraphs = service
        .call(SubgraphFetchAllRequest::new(input.graph_ref.clone()))
        .await?;
    Ok(subgraphs)
}

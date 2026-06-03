use tower::{Service, ServiceExt};

use crate::{
    blocking::StudioClient,
    operations::{
        config::is_federated::{self, IsFederatedInput},
        subgraph::check::{service::SubgraphCheck, types::SubgraphCheckAsyncInput},
    },
    shared::CheckRequestSuccessResult,
    RoverClientError,
};

/// The main function to be used from this module.
/// This function takes a proposed schema and validates it against a published
/// schema.
pub async fn run(
    input: SubgraphCheckAsyncInput,
    client: &StudioClient,
) -> Result<CheckRequestSuccessResult, RoverClientError> {
    let graph_ref = input.graph_ref.clone();
    // This response is used to check whether or not the current graph is federated.
    let is_federated = is_federated::run(
        IsFederatedInput {
            graph_ref: graph_ref.clone(),
        },
        client,
    )
    .await?;
    if !is_federated {
        return Err(RoverClientError::ExpectedFederatedGraph {
            graph_ref,
            can_operation_convert: false,
        });
    }
    let mut service = SubgraphCheck::new(
        client
            .studio_graphql_service()
            .map_err(|err| RoverClientError::ServiceReady(Box::new(err)))?,
    );
    let service = service.ready().await?;
    service.call(input).await
}

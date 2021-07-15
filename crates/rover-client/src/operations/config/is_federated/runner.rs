use crate::blocking::StudioClient;
use crate::operations::config::is_federated::IsFederatedInput;
use crate::shared::GraphRef;
use crate::RoverClientError;

use graphql_client::*;

#[derive(GraphQLQuery)]
// The paths are relative to the directory where your `Cargo.toml` is located.
// Both json and the GraphQL schema language are supported as sources for the schema
#[graphql(
    query_path = "src/operations/config/is_federated/is_federated_query.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
/// This struct is used to generate the module containing `Variables` and
/// `ResponseData` structs.
/// Snake case of this name is the mod name. i.e. publish_partial_schema_mutation
pub(crate) struct IsFederatedGraph;

pub(crate) fn run(
    input: IsFederatedInput,
    client: &StudioClient,
) -> Result<bool, RoverClientError> {
    let graph_ref = input.graph_ref.clone();
    let data = client.post::<IsFederatedGraph>(input.into())?;
    build_response(data, graph_ref)
}

type ImplementingServices = is_federated_graph::IsFederatedGraphServiceImplementingServices;

fn build_response(
    data: is_federated_graph::ResponseData,
    graph_ref: GraphRef,
) -> Result<bool, RoverClientError> {
    let service = data
        .service
        .ok_or(RoverClientError::GraphNotFound { graph_ref })?;
    Ok(match service.implementing_services {
        Some(typename) => match typename {
            ImplementingServices::FederatedImplementingServices => true,
            ImplementingServices::NonFederatedImplementingService => false,
        },
        None => false,
    })
}

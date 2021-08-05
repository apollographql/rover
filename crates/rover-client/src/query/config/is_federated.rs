// PublishPartialSchemaMutation
use crate::blocking::StudioClient;
use crate::RoverClientError;
use graphql_client::*;

#[derive(GraphQLQuery)]
// The paths are relative to the directory where your `Cargo.toml` is located.
// Both json and the GraphQL schema language are supported as sources for the schema
#[graphql(
    query_path = "src/query/config/is_federated.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
/// This struct is used to generate the module containing `Variables` and
/// `ResponseData` structs.
/// Snake case of this name is the mod name. i.e. publish_partial_schema_mutation
pub(crate) struct IsFederatedGraph;

pub(crate) fn run(
    variables: is_federated_graph::Variables,
    client: &StudioClient,
) -> Result<bool, RoverClientError> {
    let graph = variables.graph_id.clone();
    let data = client.post::<IsFederatedGraph>(variables)?;
    build_response(data, graph)
}

type ImplementingServices = is_federated_graph::IsFederatedGraphServiceImplementingServices;

fn build_response(
    data: is_federated_graph::ResponseData,
    graph: String,
) -> Result<bool, RoverClientError> {
    let service = data.service.ok_or(RoverClientError::NoService { graph })?;
    Ok(match service.implementing_services {
        Some(typename) => match typename {
            ImplementingServices::FederatedImplementingServices => true,
            ImplementingServices::NonFederatedImplementingService => false,
        },
        None => false,
    })
}

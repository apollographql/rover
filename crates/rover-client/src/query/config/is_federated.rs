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
pub struct IsFederatedGraph;

#[derive(Debug, PartialEq)]
pub struct IsFederatedGraphResponse {
    pub result: bool,
}

pub fn run(
    variables: is_federated_graph::Variables,
    client: &StudioClient,
) -> Result<IsFederatedGraphResponse, RoverClientError> {
    let data = client.post::<IsFederatedGraph>(variables)?;
    let is_federated_response = data.service.unwrap();
    Ok(build_response(is_federated_response))
}

type FederatedResponse = is_federated_graph::IsFederatedGraphService;
type ImplementingServices = is_federated_graph::IsFederatedGraphServiceImplementingServices;

fn build_response(service: FederatedResponse) -> IsFederatedGraphResponse {
    match service.implementing_services {
        Some(typename) => match typename {
            ImplementingServices::FederatedImplementingServices => {
                IsFederatedGraphResponse { result: true }
            }
            ImplementingServices::NonFederatedImplementingService => {
                IsFederatedGraphResponse { result: false }
            }
        },
        None => IsFederatedGraphResponse { result: false },
    }
}

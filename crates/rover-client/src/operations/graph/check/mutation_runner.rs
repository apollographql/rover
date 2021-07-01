use crate::blocking::StudioClient;
use crate::operations::graph::check::types::{GraphCheckInput, MutationResponseData};
use crate::shared::CheckResponse;
use crate::RoverClientError;

use graphql_client::*;

type Timestamp = String;
#[derive(GraphQLQuery)]
// The paths are relative to the directory where your `Cargo.toml` is located.
// Both json and the GraphQL schema language are supported as sources for the schema
#[graphql(
    query_path = "src/operations/graph/check/check_mutation.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
/// This struct is used to generate the module containing `Variables` and
/// `ResponseData` structs.
/// Snake case of this name is the mod name. i.e. graph_check_mutation
pub(crate) struct GraphCheckMutation;

/// The main function to be used from this module.
/// This function takes a proposed schema and validates it against a published
/// schema.
pub fn run(
    input: GraphCheckInput,
    client: &StudioClient,
) -> Result<CheckResponse, RoverClientError> {
    let graph = input.graph_id.clone();
    let data = client.post::<GraphCheckMutation>(input.into())?;
    get_check_response_from_data(data, graph)
}

fn get_check_response_from_data(
    data: MutationResponseData,
    graph: String,
) -> Result<CheckResponse, RoverClientError> {
    let service = data.service.ok_or(RoverClientError::NoService { graph })?;
    let target_url = service.check_schema.target_url;

    let diff_to_previous = service.check_schema.diff_to_previous;

    let number_of_checked_operations = diff_to_previous.number_of_checked_operations.unwrap_or(0);

    let change_severity = diff_to_previous.severity.into();
    let mut changes = Vec::with_capacity(diff_to_previous.changes.len());
    for change in diff_to_previous.changes {
        changes.push(change.into());
    }

    Ok(CheckResponse {
        target_url,
        number_of_checked_operations,
        change_severity,
        changes,
    })
}

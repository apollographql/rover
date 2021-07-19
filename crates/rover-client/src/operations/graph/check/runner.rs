use crate::blocking::StudioClient;
use crate::operations::graph::check::types::{GraphCheckInput, MutationResponseData};
use crate::shared::{CheckResponse, GraphRef};
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
    let graph_ref = input.graph_ref.clone();
    let data = client.post::<GraphCheckMutation>(input.into())?;
    get_check_response_from_data(data, graph_ref)
}

fn get_check_response_from_data(
    data: MutationResponseData,
    graph_ref: GraphRef,
) -> Result<CheckResponse, RoverClientError> {
    let service = data.service.ok_or(RoverClientError::GraphNotFound {
        graph_ref: graph_ref.clone(),
    })?;
    let target_url = service.check_schema.target_url;

    let diff_to_previous = service.check_schema.diff_to_previous;

    let operation_check_count = diff_to_previous.number_of_checked_operations.unwrap_or(0) as u64;

    let result = diff_to_previous.severity.into();
    let mut changes = Vec::with_capacity(diff_to_previous.changes.len());
    for change in diff_to_previous.changes {
        changes.push(change.into());
    }

    let check_response = CheckResponse::new(target_url, operation_check_count, changes, result);

    check_response.check_for_failures(graph_ref)
}

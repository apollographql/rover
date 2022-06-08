use crate::blocking::StudioClient;
use crate::operations::graph::check::types::{CheckSchemaAsyncInput, MutationResponseData};
use crate::operations::workflow::status::CheckWorkflowInput;
use crate::shared::{CheckResponse, GraphRef};
use crate::RoverClientError;

use graphql_client::*;

use crate::operations::graph::check::runner::graph_check_mutation::GraphCheckMutationGraphVariantSubmitCheckSchemaAsync::{CheckRequestSuccess, InvalidInputError, PermissionError, PlanError};

type Timestamp = String;

#[derive(GraphQLQuery)]
// The paths are relative to the directory where your `Cargo.toml` is located.
// Both json and the GraphQL schema language are supported as sources for the schema
#[graphql(
    query_path = "src/operations/graph/check/check_workflow_query.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
/// This struct is used to generate the module containing `Variables` and
/// `ResponseData` structs.
/// Snake case of this name is the mod name. i.e. graph_check_workflow_query
pub(crate) struct GraphCheckWorkflowQuery;

/// The main function to be used from this module.
/// This function takes a proposed schema and validates it against a published
/// schema.
pub fn run(
    input: CheckWorkflowInput,
    client: &StudioClient,
) -> Result<CheckRequestSuccessResult, RoverClientError> {
    let graph_ref = input.graph_ref.clone();
    let data = client.post::<GraphCheckWorkflowQuery>(input.into())?;
    get_check_response_from_data(data, graph_ref)
}

fn get_check_response_from_data(
    data: ResponseData,
    graph_ref: GraphRef,
) -> Result<CheckResponse, RoverClientError> {
    let graph = data.graph.ok_or(RoverClientError::GraphNotFound {
        graph_ref: graph_ref.clone(),
    })?;
}

pub fn run2(
    input: GraphCheckInput,
    client: &StudioClient,
) -> Result<CheckResponse, RoverClientError> {
    let graph_ref = input.graph_ref.clone();
    let data = client.post::<GraphCheckMutation>(input.into())?;
    get_check_response_from_data(data, graph_ref)
}

fn get_check_response_from_data2(
    data: MutationResponseData,
    graph_ref: GraphRef,
) -> Result<CheckResponse, RoverClientError> {
    let graph = data.graph.ok_or(RoverClientError::GraphNotFound {
        graph_ref: graph_ref.clone(),
    })?;
    let target_url = graph.check_schema.target_url;

    let diff_to_previous = graph.check_schema.diff_to_previous;

    let operation_check_count = diff_to_previous.number_of_checked_operations.unwrap_or(0) as u64;

    let result = diff_to_previous.severity.into();
    let mut changes = Vec::with_capacity(diff_to_previous.changes.len());
    for change in diff_to_previous.changes {
        changes.push(change.into());
    }

    // The `graph` check response does not return this field
    // only `subgraph` check does. Since `CheckResponse` is shared
    // between `graph` and `subgraph` checks, defaulting this
    // to false for now since its currently only used in
    // `check_response.rs` to format better console messages.
    let core_schema_modified = false;

    CheckResponse::try_new(
        target_url,
        operation_check_count,
        changes,
        result,
        graph_ref,
        core_schema_modified,
    )
}

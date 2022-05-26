use crate::blocking::StudioClient;
use crate::operations::graph::async_check::types::{CheckSchemaAsyncInput, MutationResponseData};
use crate::shared::{CheckRequestSuccessResult, GraphRef};
use crate::RoverClientError;

use graphql_client::*;

use crate::operations::graph::async_check::runner::graph_async_check_mutation::GraphAsyncCheckMutationGraphVariantSubmitCheckSchemaAsync::{CheckRequestSuccess, InvalidInputError, PermissionError, PlanError};

type Timestamp = String;
#[derive(GraphQLQuery)]
// The paths are relative to the directory where your `Cargo.toml` is located.
// Both json and the GraphQL schema language are supported as sources for the schema
#[graphql(
    query_path = "src/operations/graph/async_check/async_check_mutation.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
/// This struct is used to generate the module containing `Variables` and
/// `ResponseData` structs.
/// Snake case of this name is the mod name. i.e. graph_async_check_mutation
pub(crate) struct GraphAsyncCheckMutation;

/// The main function to be used from this module.
/// This function takes a proposed schema and validates it against a published
/// schema.
pub fn run(
    input: CheckSchemaAsyncInput,
    client: &StudioClient,
) -> Result<CheckRequestSuccessResult, RoverClientError> {
    let graph_ref = input.graph_ref.clone();
    let data = client.post::<GraphAsyncCheckMutation>(input.into())?;
    get_check_response_from_data(data, graph_ref)
}

fn get_check_response_from_data(
    data: MutationResponseData,
    graph_ref: GraphRef,
) -> Result<CheckRequestSuccessResult, RoverClientError> {
    let graph = data.graph.ok_or(RoverClientError::GraphNotFound {
        graph_ref: graph_ref.clone(),
    })?;
    let variant = graph.variant.ok_or(RoverClientError::GraphNotFound {
        graph_ref: graph_ref.clone(),
    })?;
    let typename = variant.submit_check_schema_async;

    match typename {
        CheckRequestSuccess(result) => Ok(CheckRequestSuccessResult {
            target_url: result.target_url,
            workflow_id: result.workflow_id,
        }),
        InvalidInputError(error) => Err(RoverClientError::InvalidInputError {
            msg: error.message,
        }),
        PermissionError(error) => Err(RoverClientError::PermissionError {
            msg: error.message,
        }),
        PlanError(error) => Err(RoverClientError::PlanError {
            msg: error.message,
        })
    }
}
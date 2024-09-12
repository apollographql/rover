use crate::blocking::StudioClient;
use crate::operations::graph::check::types::{CheckSchemaAsyncInput, MutationResponseData};
use crate::shared::{CheckRequestSuccessResult, GraphRef};
use crate::RoverClientError;

use graphql_client::*;

use crate::operations::graph::check::runner::graph_check_mutation::GraphCheckMutationGraphVariantSubmitCheckSchemaAsync::{CheckRequestSuccess, InvalidInputError, PermissionError, PlanError, RateLimitExceededError};

#[derive(GraphQLQuery)]
// The paths are relative to the directory where your `Cargo.toml` is located.
// Both json and the GraphQL schema language are supported as sources for the schema
#[graphql(
    query_path = "src/operations/graph/check/graph_check_mutation.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "Eq, PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
/// This struct is used to generate the module containing `Variables` and
/// `ResponseData` structs.
/// Snake case of this name is the mod name. i.e. graph_check_mutation
pub(crate) struct GraphCheckMutation;

/// The main function to be used from this module.
/// This function takes a proposed schema and validates it against a published
/// schema.
pub async fn run(
    input: CheckSchemaAsyncInput,
    client: &StudioClient,
) -> Result<CheckRequestSuccessResult, RoverClientError> {
    let graph_ref = input.graph_ref.clone();
    let data = client.post::<GraphCheckMutation>(input.into()).await?;
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
        InvalidInputError(..) => Err(RoverClientError::InvalidInputError { graph_ref }),
        PermissionError(error) => Err(RoverClientError::PermissionError { msg: error.message }),
        PlanError(error) => Err(RoverClientError::PlanError { msg: error.message }),
        RateLimitExceededError => Err(RoverClientError::RateLimitExceeded),
    }
}

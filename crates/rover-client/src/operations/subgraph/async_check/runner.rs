use crate::blocking::StudioClient;
use crate::operations::config::is_federated::{self, IsFederatedInput};
use crate::operations::subgraph::async_check::types::{SubgraphCheckAsyncInput, MutationResponseData};
use crate::shared::{CheckRequestSuccessResult, GraphRef};
use crate::RoverClientError;

use graphql_client::*;

use crate::operations::subgraph::async_check::runner::subgraph_async_check_mutation::SubgraphAsyncCheckMutationGraphVariantSubmitSubgraphCheckAsync::{CheckRequestSuccess, InvalidInputError, PermissionError, PlanError};

type GraphQLDocument = String;
type Timestamp = String;

#[derive(GraphQLQuery)]
// The paths are relative to the directory where your `Cargo.toml` is located.
// Both json and the GraphQL schema language are supported as sources for the schema
#[graphql(
    query_path = "src/operations/subgraph/async_check/async_check_mutation.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
/// This struct is used to generate the module containing `Variables` and
/// `ResponseData` structs.
/// Snake case of this name is the mod name. i.e. subgraph_async_check_mutation
pub(crate) struct SubgraphAsyncCheckMutation;

/// The main function to be used from this module.
/// This function takes a proposed schema and validates it against a published
/// schema.
pub fn run(
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
    )?;
    if !is_federated {
        return Err(RoverClientError::ExpectedFederatedGraph {
            graph_ref,
            can_operation_convert: false,
        });
    }
    let data = client.post::<SubgraphAsyncCheckMutation>(input.into())?;
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
    let typename = variant.submit_subgraph_check_async;

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
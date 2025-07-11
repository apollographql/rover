use crate::blocking::StudioClient;
use crate::operations::config::is_federated::{self, IsFederatedInput};
use crate::operations::supergraph::check::types::{ResponseData, SupergraphCheckInput};
use crate::shared::{CheckRequestSuccessResult, GraphRef};
use crate::RoverClientError;

use graphql_client::*;

use crate::operations::supergraph::check::runner::supergraph_check_mutation::SupergraphCheckMutationGraphVariantSubmitMultiSubgraphCheckAsync::{
    CheckRequestSuccess, InvalidInputError, PermissionError, PlanError, RateLimitExceededError,
};

type GraphQLDocument = String;

#[derive(GraphQLQuery)]
// The paths are relative to the directory where your `Cargo.toml` is located.
// Both json and the GraphQL schema language are supported as sources for the schema
#[graphql(
    query_path = "src/operations/supergraph/check/supergraph_check_mutation.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "Eq, PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
/// This struct is used to generate the module containing `Variables` and
/// `ResponseData` structs.
/// Snake case of this name is the mod name. i.e. supergraph_check_mutation
pub(crate) struct SupergraphCheckMutation;

/// The main function to be used from this module.
/// This function takes a proposed schema and validates it against a published
/// schema.
pub async fn run(
    input: SupergraphCheckInput,
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
    let data = client.post::<SupergraphCheckMutation>(input.into()).await?;
    get_check_response_from_data(data, graph_ref)
}

fn get_check_response_from_data(
    data: ResponseData,
    graph_ref: GraphRef,
) -> Result<CheckRequestSuccessResult, RoverClientError> {
    let graph = data.graph.ok_or(RoverClientError::GraphNotFound {
        graph_ref: graph_ref.clone(),
    })?;
    let variant = graph.variant.ok_or(RoverClientError::GraphNotFound {
        graph_ref: graph_ref.clone(),
    })?;
    let typename = variant.submit_multi_subgraph_check_async;

    match typename {
        CheckRequestSuccess(result) => Ok(CheckRequestSuccessResult {
            target_url: result.target_url,
            workflow_id: result.workflow_id,
        }),
        InvalidInputError(..) => Err(RoverClientError::InvalidInputError { graph_ref }),
        PermissionError(error) => Err(RoverClientError::PermissionError { msg: error.message }),
        PlanError(error) => Err(RoverClientError::PlanError { msg: error.message }),
        RateLimitExceededError(_) => Err(RoverClientError::RateLimitExceeded),
    }
}

use crate::blocking::StudioClient;
use crate::operations::config::is_federated::{self, IsFederatedInput};
use crate::operations::subgraph::fetch;
use crate::operations::subgraph::fetch::SubgraphFetchInput;
use crate::operations::subgraph::lint::types::{LintSubgraphInput, LintSubgraphMutationInput};
use crate::RoverClientError;

use graphql_client::*;

#[derive(GraphQLQuery)]
// The paths are relative to the directory where your `Cargo.toml` is located.
// Both json and the GraphQL schema language are supported as sources for the schema
#[graphql(
    query_path = "src/operations/subgraph/lint/lint_subgraph_mutation.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "Eq, PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
/// This struct is used to generate the module containing `Variables` and
/// `ResponseData` structs.
/// Snake case of this name is the mod name. i.e. lint_subgraph_mutation
pub(crate) struct LintSubgraphMutation;

/// The main function to be used from this module.
/// This function takes a proposed schema and validates it against a published
/// schema.
pub fn run(
    input: LintSubgraphInput,
    client: &StudioClient,
) -> Result<LintResponse, RoverClientError> {
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

    let base_schema = if input.ignore_existing {
        let fetch_response = fetch::run(
            SubgraphFetchInput {
                graph_ref: graph_ref.clone(),
                subgraph_name: input.subgraph_name,
            },
            client,
        )?;
        Some(fetch_response.sdl.contents)
    } else {
        None
    };

    let data = client.post::<LintSubgraphMutation>(
        LintSubgraphMutationInput {
            graph_ref,
            proposed_schema: input.proposed_schema,
            base_schema,
        }
        .into(),
    )?;

    Ok(LintResponse {
        result: serde_json::to_string(&data)?,
    })
}

// Replace with response output object
pub struct LintResponse {
    pub result: String,
}

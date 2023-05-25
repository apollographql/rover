use crate::blocking::StudioClient;
use crate::operations::config::is_federated::{self, IsFederatedInput};
use crate::operations::subgraph::lint::types::{LintSubgraphInput, LintSubgraphMutationInput, SubgraphFetchInput, SubgraphFetchQueryVariant, SubgraphFetchResponseData};
use crate::RoverClientError;

use graphql_client::*;

#[derive(GraphQLQuery)]
// The paths are relative to the directory where your `Cargo.toml` is located.
// Both json and the GraphQL schema language are supported as sources for the schema
#[graphql(
    query_path = "src/operations/subgraph/lint/lint_schema_mutation.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "Eq, PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
/// This struct is used to generate the module containing `Variables` and
/// `ResponseData` structs.
/// Snake case of this name is the mod name. i.e. lint_schema_mutation
pub(crate) struct LintSchemaMutation;

#[derive(GraphQLQuery)]
// The paths are relative to the directory where your `Cargo.toml` is located.
// Both json and the GraphQL schema language are supported as sources for the schema
#[graphql(
    query_path = "src/operations/subgraph/lint/fetch_subgraph_query.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "Eq, PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
/// This struct is used to generate the module containing `Variables` and
/// `ResponseData` structs.
/// Snake case of this name is the mod name. i.e. subgraph_fetch_query
pub(crate) struct SubgraphFetchQuery;

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
            graph_ref: input.graph_ref,
            can_operation_convert: false,
        });
    }

    let base_schema_response = client.post::<SubgraphFetchQuery>(SubgraphFetchInput {
        graph_ref: graph_ref.clone(),
        subgraph_name: input.subgraph_name,
    }.into())?;

    let base_schema = get_sdl_from_response_data(base_schema_response).ok();

    let data = client.post::<LintSchemaMutation>(LintSubgraphMutationInput {
        graph_ref: graph_ref.clone(),
        proposed_schema: input.proposed_schema,
        base_schema: base_schema,
    }.into())?;

    return Ok(LintResponse { result: serde_json::to_string(&data)? });
}


fn get_sdl_from_response_data(
    base_schema_response: SubgraphFetchResponseData
) -> Result<String, RoverClientError> {
    if let Some(maybe_variant) = base_schema_response.variant {
        match maybe_variant {
            SubgraphFetchQueryVariant::GraphVariant(variant) => {
                if let Some(subgraph) = variant.subgraph {
                    Ok(subgraph.active_partial_schema.sdl)
                } else {
                    Err(RoverClientError::InvalidGraphRef)
                }
            }
            SubgraphFetchQueryVariant::InvalidRefFormat => {
                Err(RoverClientError::InvalidGraphRef)
            }
        }
    } else {
        Err(RoverClientError::InvalidGraphRef)
    }
}

// Replace with response output object
pub struct LintResponse {
    pub result: String,
}

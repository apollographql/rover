use crate::blocking::StudioClient;
use crate::operations::graph::lint::types::{LintGraphInput, LintGraphMutationInput, GraphFetchInput, GraphFetchResponseData};
use crate::RoverClientError;

use graphql_client::*;

/// this is because of the custom GraphQLDocument scalar in the schema
type GraphQLDocument = String;

#[derive(GraphQLQuery)]
// The paths are relative to the directory where your `Cargo.toml` is located.
// Both json and the GraphQL schema language are supported as sources for the schema
#[graphql(
    query_path = "src/operations/graph/lint/lint_schema_mutation.graphql",
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
    query_path = "src/operations/graph/lint/fetch_graph_query.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "Eq, PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
/// This struct is used to generate the module containing `Variables` and
/// `ResponseData` structs.
/// Snake case of this name is the mod name. i.e. graph_fetch_query
pub(crate) struct GraphFetchQuery;

/// The main function to be used from this module.
/// This function takes a proposed schema and validates it against a published
/// schema.
pub fn run(
    input: LintGraphInput,
    client: &StudioClient,
) -> Result<LintResponse, RoverClientError> {
    let graph_ref = input.graph_ref.clone();

    let base_schema_response = client.post::<GraphFetchQuery>(GraphFetchInput {
        graph_ref: graph_ref.clone(),
    }.into())?;

    let base_schema = get_sdl_from_response_data(base_schema_response).ok();

    let data = client.post::<LintSchemaMutation>(LintGraphMutationInput {
        graph_ref: graph_ref.clone(),
        proposed_schema: input.proposed_schema,
        base_schema: base_schema,
    }.into())?;

    return Ok(LintResponse { result: serde_json::to_string(&data)? });
}

fn get_sdl_from_response_data(
    base_schema_response: GraphFetchResponseData
) -> Result<String, RoverClientError> {
    if let Some(maybe_graph) = base_schema_response.graph {
        if let Some(maybe_variant) = maybe_graph.variant {
            if let Some(maybe_publication) = maybe_variant.latest_publication {
                Ok(maybe_publication.schema.document)
            } else {
            Err(RoverClientError::InvalidGraphRef)
        }
        } else {
            Err(RoverClientError::InvalidGraphRef)
        }
    } else {
        Err(RoverClientError::InvalidGraphRef)
    }
}

// Replace with response output object
pub struct LintResponse {
    pub result: String,
}

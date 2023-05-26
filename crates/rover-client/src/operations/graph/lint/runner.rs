use crate::blocking::StudioClient;
use crate::operations::graph::fetch;
use crate::operations::graph::fetch::GraphFetchInput;
use crate::operations::graph::lint::types::{LintGraphInput, LintGraphMutationInput};
use crate::RoverClientError;

use graphql_client::*;

#[derive(GraphQLQuery)]
// The paths are relative to the directory where your `Cargo.toml` is located.
// Both json and the GraphQL schema language are supported as sources for the schema
#[graphql(
    query_path = "src/operations/graph/lint/lint_graph_mutation.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "Eq, PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
/// This struct is used to generate the module containing `Variables` and
/// `ResponseData` structs.
/// Snake case of this name is the mod name. i.e. lint_graph_mutation
pub(crate) struct LintGraphMutation;

/// The main function to be used from this module.
/// This function takes a proposed schema and validates it against a published
/// schema.
pub fn run(input: LintGraphInput, client: &StudioClient) -> Result<LintResponse, RoverClientError> {
    let graph_ref = input.graph_ref.clone();

    let base_schema = if input.ignore_existing {
        let fetch_response = fetch::run(
            GraphFetchInput {
                graph_ref: graph_ref.clone(),
            },
            client,
        )?;
        Some(fetch_response.sdl.contents)
    } else {
        None
    };

    let data = client.post::<LintGraphMutation>(
        LintGraphMutationInput {
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

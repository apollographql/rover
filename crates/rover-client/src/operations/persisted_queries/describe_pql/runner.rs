use super::types::{DescribePQLInput, DescribePQLResponse};
use crate::blocking::StudioClient;
use crate::shared::GraphRef;
use crate::RoverClientError;
use graphql_client::*;

#[derive(GraphQLQuery, Debug)]
// The paths are relative to the directory where your `Cargo.toml` is located.
// Both json and the GraphQL schema language are supported as sources for the schema
#[graphql(
    query_path = "src/operations/persisted_queries/describe_pql/describe_pql_query.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "Eq, PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
pub struct DescribePersistedQueryListQuery;

pub fn run(
    input: DescribePQLInput,
    client: &StudioClient,
) -> Result<DescribePQLResponse, RoverClientError> {
    let graph_ref = input.graph_ref.clone();
    let data = client.post::<DescribePersistedQueryListQuery>(input.into())?;
    build_response(data, graph_ref)
}

fn build_response(
    data: describe_persisted_query_list_query::ResponseData,
    graph_ref: GraphRef,
) -> Result<DescribePQLResponse, RoverClientError> {
    let graph = data.graph.ok_or(RoverClientError::GraphNotFound {
        graph_ref: graph_ref.clone(),
    })?;

    let valid_variants = graph
        .variants
        .iter()
        .map(|variant| variant.name.clone())
        .collect();

    let variant = graph.variant.ok_or(RoverClientError::NoSchemaForVariant {
        graph_ref: graph_ref.clone(),
        valid_variants,
        frontend_url_root: data.frontend_url_root,
    })?;

    if let Some(list) = variant.persisted_query_list {
        Ok(DescribePQLResponse {
            graph_ref,
            id: list.id,
        })
    } else {
        // FIXME: make a real error, provide a way to fix the error
        Err(RoverClientError::AdhocError {
            msg: format!("could not find a persisted query list linked to {graph_ref}"),
        })
    }
}

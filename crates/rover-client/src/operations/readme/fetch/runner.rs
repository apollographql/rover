use crate::blocking::StudioClient;
use crate::operations::readme::fetch::ReadmeFetchInput;
use crate::shared::GraphRef;
use crate::RoverClientError;
use graphql_client::*;

#[derive(GraphQLQuery)]
// The paths are relative to the directory where your `Cargo.toml` is located.
// Both json and the GraphQL schema language are supported as sources for the schema
#[graphql(
    query_path = "src/operations/readme/fetch/fetch_query.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
pub struct ReadmeFetchQuery;

pub fn run(input: ReadmeFetchInput, client: &StudioClient) -> Result<String, RoverClientError> {
    let graph_ref = input.graph_ref.clone();
    let data = client.post::<ReadmeFetchQuery>(input.into())?;
    build_response(data, graph_ref)
}

fn build_response(
    data: readme_fetch_query::ResponseData,
    graph_ref: GraphRef,
) -> Result<String, RoverClientError> {
    let graph = data.graph.ok_or(RoverClientError::GraphNotFound {
        graph_ref: graph_ref.clone(),
    })?;
    let variant = graph.variant.ok_or(RoverClientError::GraphNotFound {
        graph_ref: graph_ref.clone(),
    })?;
    let readme = variant.readme;
    match readme {
        Some(v) => Ok(v.content),
        None => Ok("No README defined".to_string()),
    }
}

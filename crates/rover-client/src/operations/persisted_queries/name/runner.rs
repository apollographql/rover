use crate::blocking::StudioClient;
use crate::operations::persisted_queries::name::{
    PersistedQueryListNameInput, PersistedQueryListNameResponse,
};
use crate::RoverClientError;
use graphql_client::*;

#[derive(GraphQLQuery, Debug)]
// The paths are relative to the directory where your `Cargo.toml` is located.
// Both json and the GraphQL schema language are supported as sources for the schema
#[graphql(
    query_path = "src/operations/persisted_queries/name/name_query.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "Eq, PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
pub struct PersistedQueryListNameQuery;

pub async fn run(
    input: PersistedQueryListNameInput,
    client: &StudioClient,
) -> Result<PersistedQueryListNameResponse, RoverClientError> {
    let graph_id = input.graph_id.clone();
    let list_id = input.list_id.clone();
    let data = client
        .post::<PersistedQueryListNameQuery>(input.into())
        .await?;
    build_response(data, graph_id, list_id)
}

fn build_response(
    data: persisted_query_list_name_query::ResponseData,
    graph_id: String,
    list_id: String,
) -> Result<PersistedQueryListNameResponse, RoverClientError> {
    let graph = data.graph.ok_or(RoverClientError::GraphIdNotFound {
        graph_id: graph_id.clone(),
    })?;

    let persisted_query_list =
        graph
            .persisted_query_list
            .ok_or(RoverClientError::PersistedQueryListIdNotFound {
                graph_id,
                list_id,
                frontend_url_root: data.frontend_url_root,
            })?;

    Ok(PersistedQueryListNameResponse {
        name: persisted_query_list.name,
    })
}

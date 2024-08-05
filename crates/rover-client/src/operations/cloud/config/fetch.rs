use crate::blocking::StudioClient;
use crate::operations::cloud::config::types::CloudConfigFetchInput;
use crate::RoverClientError;
use graphql_client::*;

#[derive(GraphQLQuery, Debug)]
// The paths are relative to the directory where your `Cargo.toml` is located.
// Both json and the GraphQL schema language are supported as sources for the schema
#[graphql(
    query_path = "src/operations/cloud/config/fetch_query.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "Eq, PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
pub struct CloudConfigFetchQuery;

pub fn fetch(input: CloudConfigFetchInput, client: &StudioClient) -> Result<(), RoverClientError> {
    let graph_ref = input.graph_ref.clone();
    let data = client.post::<CloudConfigFetchQuery>(input.into())?;
    Ok(())
}

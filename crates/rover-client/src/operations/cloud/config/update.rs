use crate::blocking::StudioClient;
use crate::operations::cloud::config::types::CloudConfigUpdateInput;
use crate::RoverClientError;
use graphql_client::*;

#[derive(GraphQLQuery, Debug)]
// The paths are relative to the directory where your `Cargo.toml` is located.
// Both json and the GraphQL schema language are supported as sources for the schema
#[graphql(
    query_path = "src/operations/cloud/config/update_query.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "Eq, PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
pub struct CloudConfigUpdateQuery;

pub fn update(
    input: CloudConfigUpdateInput,
    client: &StudioClient,
) -> Result<(), RoverClientError> {
    let graph_ref = input.graph_ref.clone();
    let data = client.post::<CloudConfigUpdateQuery>(input.into())?;
    Ok(())
}

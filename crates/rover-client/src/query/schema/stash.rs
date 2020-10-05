use crate::blocking::Client;
use crate::RoverClientError;
use graphql_client::*;

#[derive(GraphQLQuery)]
// The paths are relative to the directory where your `Cargo.toml` is located.
// Both json and the GraphQL schema language are supported as sources for the schema
#[graphql(
    query_path = "src/query/schema/stash.graphql",
    schema_path = "schema.graphql",
    response_derives = "PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
/// This struct is used to generate the module containing `Variables` and
/// `ResponseData` structs.
/// Snake case of this name is the mod name. i.e. stash_schema_query
pub struct StashSchemaMutation;

/// TODO
pub fn run(
    variables: stash_schema_mutation::Variables,
    client: Client,
) -> Result<String, RoverClientError> {
    let res = client.post::<StashSchemaMutation>(variables);

    let data = res.expect("Invalid service id or api key");
    let data = data.expect("Invalid service id or api key");
    let data = data.service.expect("Invalid service id or api key");
    let data = data.upload_schema.expect("No response from update schema mutation");
    
    if !data.success {
        panic!("Upload failed for following reason: {}", data.message);
    }

    let hash = data.tag.expect("No schema info in response from schema update");
    let hash = hash.schema.hash;
    
    Ok(hash)
}

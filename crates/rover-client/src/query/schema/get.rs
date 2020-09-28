

use graphql_client::*;
use crate::blocking::Client;

// I'm not sure where this should live long-term
/// this is because of the custom GraphQLDocument scalar in the schema
type GraphQLDocument = String;

#[derive(GraphQLQuery)]
// The paths are relative to the directory where your `Cargo.toml` is located.
// Both json and the GraphQL schema language are supported as sources for the schema
#[graphql(
    query_path = "src/query/schema/get.graphql",
    schema_path = "schema.graphql",
    response_derives = "PartialEq, Debug",
    deprecated = "warn"
)]
/// TODO: doc difference in this and get_schema_query
pub struct GetSchemaQuery;

/// The main function to be used from this module. This function "executes" the
/// `get` functionality from apollo studio
pub fn execute(variables: get_schema_query::Variables) -> Result<(), ()>{
    // TODO: where do we get the api_key and uri from? do we need a default uri?
    // we need to be able to pass uri from config
    let client = Client::new("".to_string(), None);
    let res = client.post(variables);
    Ok(())
}
use graphql_schema::schema::Schema;
use introspection::introspection_response::IntrospectionResponse as Response;
use serde::Deserialize;
use std::collections::HashMap;

use crate::blocking::Client;
use crate::RoverClientError;
use graphql_client::*;

#[derive(GraphQLQuery)]
// The paths are relative to the directory where your `Cargo.toml` is located.
// Both json and the GraphQL schema language are supported as sources for the schema
#[graphql(
    query_path = "src/query/graph/introspect_query.graphql",
    schema_path = "src/query/graph/introspect_schema.graphql",
    response_derives = "PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
/// This struct is used to generate the module containing `Variables` and
/// `ResponseData` structs.
/// Snake case of this name is the mod name. i.e. introspection_query
pub struct IntrospectionQuery;

#[derive(Debug, PartialEq, Deserialize)]
pub struct IntrospectionResponse {
    pub result: String,
}

/// The main function to be used from this module. This function fetches a
/// schema from apollo studio and returns it in either sdl (default) or json format
pub fn run(client: &Client) -> Result<Schema, RoverClientError> {
    let variables = introspection_query::Variables {};
    let response_data = client.post::<IntrospectionQuery>(variables, &HashMap::new())?;
    let response_string = serde_json::to_string(&response_data)?;
    let response_json: Response = serde_json::from_str(&response_string)?;
    Ok(build_response(response_json))
}

fn build_response(response: Response) -> Schema {
    let s = Schema::from(response);
    // dbg!("{:?}", &s);
    s
}

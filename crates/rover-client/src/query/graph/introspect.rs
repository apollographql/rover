use ::introspection_query;
use graphql_schema::schema::Schema;
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
#[derive(Display)]
pub struct IntrospectionQuery;

#[derive(Debug, PartialEq, Deserialize)]
pub struct IntrospectionResponse {
    pub result: String,
}

/// The main function to be used from this module. This function fetches a
/// schema from apollo studio and returns it in either sdl (default) or json format
pub fn run(client: &Client) -> Result<Schema, RoverClientError> {
    let variables = introspection_query::Variables {};
    let response_data = client
        .post::<IntrospectionQuery>(variables, &HashMap::new())?
        .to_string();
    let response_json = serde_json::from_str(response_data)?;
    Ok(build_response(response_json))
}

fn build_response(
    response: ::introspection_query::introspection_response::IntrospectionResponse,
) -> Schema {
    let s = Schema::from(response);
    dbg!("{:?}", s);
    s
}

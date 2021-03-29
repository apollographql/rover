use crate::blocking::Client;
use crate::RoverClientError;
use graphql_client::*;
use std::collections::HashMap;

#[derive(GraphQLQuery)]
#[graphql(
    query_path = "src/query/subgraph/introspect_query.graphql",
    schema_path = "src/query/subgraph/introspect_schema.graphql",
    response_derives = "PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]

pub struct IntrospectionQuery;

#[derive(Debug, PartialEq)]
pub struct IntrospectionResponse {
    pub result: String,
}

pub fn run(
    client: &Client,
    headers: &HashMap<String, String>,
) -> Result<IntrospectionResponse, RoverClientError> {
    // let graph = variables.graph_id.clone();
    let variables = introspection_query::Variables {};
    let response_data = client.post::<IntrospectionQuery>(variables, headers)?;
    build_response(response_data)
}

fn build_response(
    response: introspection_query::ResponseData,
) -> Result<IntrospectionResponse, RoverClientError> {
    let service_data = match response.service {
        Some(data) => Ok(data),
        None => Err(RoverClientError::IntrospectionError {
            msg: "No introspection response available.".to_string(),
        }),
    }?;

    Ok(IntrospectionResponse {
        result: service_data.sdl,
    })
}

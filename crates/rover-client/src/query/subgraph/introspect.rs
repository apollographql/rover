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
    let response_data = client.post::<IntrospectionQuery>(variables, headers);
    match response_data {
        Ok(data) => build_response(data),
        Err(e) => {
            // this is almost definitely a result of a graph not
            // being federated, or not matching the federation spec
            if e.to_string().contains("Cannot query field") {
                Err(RoverClientError::SubgraphIntrospectionNotAvailable)
            } else {
                Err(e)
            }
        }
    }
    // build_response(response_data)
}

fn build_response(
    data: introspection_query::ResponseData,
) -> Result<IntrospectionResponse, RoverClientError> {
    let service_data = data.service.ok_or(RoverClientError::IntrospectionError {
        msg: "No introspection response available.".to_string(),
    })?;

    Ok(IntrospectionResponse {
        result: service_data.sdl,
    })
}

use crate::blocking::StudioClient;
use crate::RoverClientError;
use graphql_client::*;

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

pub fn run(client: &StudioClient) -> Result<IntrospectionResponse, RoverClientError> {
    // let graph = variables.graph_id.clone();
    let variables = introspection_query::Variables {};
    let response_data = client.post::<IntrospectionQuery>(variables)?;
    build_response(response_data)
}

fn build_response(
    response: introspection_query::ResponseData,
    // graph: String,
) -> Result<IntrospectionResponse, RoverClientError> {
    let service_data = match response.service {
        Some(data) => Ok(data),
        None => Err(RoverClientError::NoService {
            graph: "Graph".to_string(),
        }),
    }?;

    println!("{:?}", service_data);
    todo!();
}

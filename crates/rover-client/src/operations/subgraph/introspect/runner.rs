use crate::blocking::GraphQLClient;
use crate::operations::subgraph::introspect::types::*;
use crate::RoverClientError;

use graphql_client::*;

#[derive(GraphQLQuery)]
#[graphql(
    query_path = "src/operations/subgraph/introspect/introspect_query.graphql",
    schema_path = "src/operations/subgraph/introspect/introspect_schema.graphql",
    response_derives = "PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]

pub(crate) struct SubgraphIntrospectQuery;

pub fn run(
    input: SubgraphIntrospectInput,
    client: &GraphQLClient,
) -> Result<SubgraphIntrospectResponse, RoverClientError> {
    let response_data =
        client.post::<SubgraphIntrospectQuery>(input.clone().into(), &input.headers);

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
}

fn build_response(data: QueryResponseData) -> Result<SubgraphIntrospectResponse, RoverClientError> {
    let service_data = data.service.ok_or(RoverClientError::IntrospectionError {
        msg: "No introspection response available.".to_string(),
    })?;

    Ok(SubgraphIntrospectResponse {
        result: service_data.sdl,
    })
}

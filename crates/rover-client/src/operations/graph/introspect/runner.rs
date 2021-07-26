use crate::blocking::GraphQLClient;
use crate::operations::graph::introspect::{types::*, Schema};
use crate::RoverClientError;
use graphql_client::*;

use std::convert::TryFrom;

#[derive(GraphQLQuery)]
#[graphql(
    query_path = "src/operations/graph/introspect/introspect_query.graphql",
    schema_path = "src/operations/graph/introspect/introspect_schema.graphql",
    response_derives = "PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]

/// This struct is used to generate the module containing `Variables` and
/// `ResponseData` structs.
/// Snake case of this name is the mod name. i.e. graph_introspect_query
pub(crate) struct GraphIntrospectQuery;

/// The main function to be used from this module. This function fetches a
/// schema from apollo studio and returns it in either sdl (default) or json format
pub fn run(
    input: GraphIntrospectInput,
    client: &GraphQLClient,
) -> Result<GraphIntrospectResponse, RoverClientError> {
    let variables = input.clone().into();
    let response_data = client.post::<GraphIntrospectQuery>(variables, &input.headers)?;
    build_response(response_data)
}

fn build_response(
    response: QueryResponseData,
) -> Result<GraphIntrospectResponse, RoverClientError> {
    match Schema::try_from(response) {
        Ok(schema) => Ok(GraphIntrospectResponse {
            schema_sdl: schema.encode(),
        }),
        Err(msg) => Err(RoverClientError::IntrospectionError { msg: msg.into() }),
    }
}

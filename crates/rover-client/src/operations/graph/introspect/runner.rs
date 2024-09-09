use crate::blocking::GraphQLClient;
use crate::error::{EndpointKind, RoverClientError};
use crate::operations::graph::introspect::{types::*, Schema};

use graphql_client::GraphQLQuery;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};

use std::convert::{Into, TryFrom};

#[derive(GraphQLQuery)]
#[graphql(
    query_path = "src/operations/graph/introspect/introspect_query.graphql",
    schema_path = "src/operations/graph/introspect/introspect_schema.graphql",
    response_derives = "PartialEq, Eq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]

/// This struct is used to generate the module containing `Variables` and
/// `ResponseData` structs.
/// Snake case of this name is the mod name. i.e. graph_introspect_query
pub(crate) struct GraphIntrospectQuery;

/// The main function to be used from this module. This function fetches a
/// schema from apollo studio and returns it in either sdl (default) or json format
pub async fn run(
    input: GraphIntrospectInput,
    client: &GraphQLClient,
    should_retry: bool,
) -> Result<GraphIntrospectResponse, RoverClientError> {
    let variables = input.clone().into();
    let mut header_map = HeaderMap::new();
    for (header_key, header_value) in input.headers {
        header_map.insert(
            HeaderName::from_bytes(header_key.as_bytes())?,
            HeaderValue::from_str(&header_value)?,
        );
    }
    let response_data = if should_retry {
        client
            .post::<GraphIntrospectQuery>(variables, &mut header_map, EndpointKind::Customer)
            .await
    } else {
        client
            .post_no_retry::<GraphIntrospectQuery>(
                variables,
                &mut header_map,
                EndpointKind::Customer,
            )
            .await
    }?;

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

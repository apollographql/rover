use reqwest::header::{HeaderMap, HeaderName, HeaderValue};

use crate::blocking::GraphQLClient;
use crate::error::EndpointKind;
use crate::operations::subgraph::introspect::types::*;
use crate::RoverClientError;

use graphql_client::*;

#[derive(GraphQLQuery)]
#[graphql(
    query_path = "src/operations/subgraph/introspect/introspect_query.graphql",
    schema_path = "src/operations/subgraph/introspect/introspect_schema.graphql",
    response_derives = "Eq, PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
pub(crate) struct SubgraphIntrospectQuery;

pub async fn run(
    input: SubgraphIntrospectInput,
    client: &GraphQLClient,
    should_retry: bool,
) -> Result<SubgraphIntrospectResponse, RoverClientError> {
    let mut header_map = HeaderMap::new();
    for (header_key, header_value) in input.clone().headers {
        header_map.insert(
            HeaderName::from_bytes(header_key.as_bytes())?,
            HeaderValue::from_str(&header_value)?,
        );
    }
    let response_data = if should_retry {
        client
            .post::<SubgraphIntrospectQuery>(input.into(), &mut header_map, EndpointKind::Customer)
            .await
    } else {
        client
            .post_no_retry::<SubgraphIntrospectQuery>(
                input.into(),
                &mut header_map,
                EndpointKind::Customer,
            )
            .await
    };

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
    let graph = data.service.ok_or(RoverClientError::IntrospectionError {
        msg: "No introspection response available.".to_string(),
    })?;

    Ok(SubgraphIntrospectResponse { result: graph.sdl })
}

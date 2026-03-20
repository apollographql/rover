use reqwest::header::{HeaderMap, HeaderName, HeaderValue};

use super::service::GraphIntrospectQuery;
use crate::{
    blocking::GraphQLClient,
    error::{EndpointKind, RoverClientError},
    operations::graph::introspect::types::*,
};

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

    super::service::build_response(response_data).map_err(RoverClientError::from)
}

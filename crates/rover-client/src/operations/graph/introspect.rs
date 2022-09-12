use crate::{blocking::GraphQLClient, RoverClientError};
use introspector_gadget::introspect::run as introspect;
pub use introspector_gadget::introspect::{GraphIntrospectInput, GraphIntrospectResponse, Schema};

/// Runs the introspection query
pub fn run(
    input: GraphIntrospectInput,
    client: &GraphQLClient,
    should_retry: bool,
) -> Result<GraphIntrospectResponse, RoverClientError> {
    introspect(input, client, should_retry).map_err(RoverClientError::from)
}

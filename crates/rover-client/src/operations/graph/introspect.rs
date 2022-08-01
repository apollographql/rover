use crate::{blocking::GraphQLClient, RoverClientError};
use launchpad::introspect::run as launchpad_run;
pub use launchpad::introspect::{GraphIntrospectInput, GraphIntrospectResponse, Schema};

/// Runs the introspection query
pub fn run(
    input: GraphIntrospectInput,
    client: &GraphQLClient,
    should_retry: bool,
) -> Result<GraphIntrospectResponse, RoverClientError> {
    launchpad_run(input, client, should_retry).map_err(|e| RoverClientError::from(e))
}

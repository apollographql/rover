use tower::{Service, ServiceExt};

use super::service::{WhoAmI, WhoAmIRequest};
use crate::{
    blocking::StudioClient, operations::config::who_am_i::types::RegistryIdentity, RoverClientError,
};

/// Get info from the registry about an API key, i.e. the name/id of the
/// user/graph and what kind of key it is (GRAPH/USER/Other)
pub async fn run(client: &StudioClient) -> Result<RegistryIdentity, RoverClientError> {
    let mut service = WhoAmI::new(
        client
            .studio_graphql_service()
            .map_err(|err| RoverClientError::ServiceReady(Box::new(err)))?,
    );
    let service = service.ready().await?;
    let identity = service
        .call(WhoAmIRequest::new(client.get_credential_origin()))
        .await?;
    Ok(identity)
}

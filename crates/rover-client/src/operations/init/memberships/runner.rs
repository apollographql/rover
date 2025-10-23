use tower::{Service, ServiceExt};

use super::service::{Memberships, MembershipsRequest};
use crate::{
    blocking::StudioClient, operations::init::memberships::types::InitMembershipsResponse,
    RoverClientError,
};

/// Get info from the registry about the user's memberships, i.e. the name/id of each of
/// the organizations the user is a member of
pub async fn run(client: &StudioClient) -> Result<InitMembershipsResponse, RoverClientError> {
    let mut service = Memberships::new(
        client
            .studio_graphql_service()
            .map_err(|err| RoverClientError::ServiceReady(Box::new(err)))?,
    );
    let service = service.ready().await?;
    let identity = service
        .call(MembershipsRequest::new(client.get_credential_origin()))
        .await?;
    Ok(identity)
}

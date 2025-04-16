use crate::options::ProfileOpt;
use crate::utils::client::StudioClientConfig;
use crate::RoverResult;
use rover_client::shared::GraphRef;
use thiserror::Error;

const DEFAULT_VARIANT: &str = "current";

#[derive(Debug, Error)]
pub enum GraphOperationError {
    #[error("Failed to authenticate with GraphOS")]
    AuthenticationFailed,
    #[error("Invalid graph ID: {0}")]
    InvalidGraphId(String),
    #[error("Failed to create API key: {0}")]
    KeyCreationFailed(String),
}

pub(crate) async fn create_api_key(
    client_config: &StudioClientConfig,
    profile: &ProfileOpt,
    graph_id: String,
    key_name: String,
) -> RoverResult<String> {
    let client = client_config
        .get_authenticated_client(profile)
        .map_err(|_| GraphOperationError::AuthenticationFailed)?;

    let graph_ref = GraphRef::new(graph_id.clone(), Some(DEFAULT_VARIANT.to_string()))
        .map_err(|_| GraphOperationError::InvalidGraphId(graph_id))?;

    let key_input = rover_client::operations::init::key::InitNewKeyInput {
        graph_ref,
        key_name,
        role: rover_client::operations::init::key::UserPermission::GraphAdmin,
    };

    let key_response = rover_client::operations::init::key::run(key_input, &client)
        .await
        .map_err(|e| GraphOperationError::KeyCreationFailed(e.to_string()))?;

    Ok(key_response.token)
}

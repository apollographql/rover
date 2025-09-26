use graphql_client::GraphQLQuery;
use serde::{Deserialize, Serialize};

use crate::blocking::StudioClient;
use crate::RoverClientError;
use crate::RoverClientError::OrganizationIDNotFound;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Eq)]
struct Void {}

#[derive(GraphQLQuery, Debug)]
#[graphql(
    query_path = "src/operations/api_keys/delete/delete_key_mutation.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "Eq, PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
struct DeleteKeyMutation;

pub struct DeleteKeyInput {
    pub organization_id: String,
    pub key_id: String,
}

impl From<DeleteKeyInput> for delete_key_mutation::Variables {
    fn from(value: DeleteKeyInput) -> Self {
        delete_key_mutation::Variables {
            key_id: value.key_id,
            organization_id: value.organization_id,
        }
    }
}

pub struct DeleteKeyResponse {
    pub key_id: String,
}

pub async fn run(
    input: DeleteKeyInput,
    client: &StudioClient,
) -> Result<DeleteKeyResponse, RoverClientError> {
    let organization_id = input.organization_id.clone();
    let key_id = input.key_id.clone();
    let data = client.post::<DeleteKeyMutation>(input.into()).await?;
    match data
        .organization
        .ok_or_else(|| OrganizationIDNotFound { organization_id })?
        .delete_key
    {
        None => Err(RoverClientError::InvalidKey),
        Some(_) => Ok(DeleteKeyResponse { key_id }),
    }
}

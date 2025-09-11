use chrono::DateTime;
use graphql_client::GraphQLQuery;

use crate::blocking::StudioClient;
use crate::operations::api_keys::get::get_key_query::GetKeyQueryOrganizationApiKey;
use crate::operations::api_keys::list::ApiKey;
use crate::RoverClientError;
use crate::RoverClientError::OrganizationIDNotFound;

type Timestamp = String;

#[derive(GraphQLQuery, Debug)]
#[graphql(
    query_path = "src/operations/api_keys/get/get_key_query.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "Eq, PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
struct GetKeyQuery;

pub struct GetKeyInput {
    pub organization_id: String,
    pub key_id: String,
}

impl From<GetKeyInput> for get_key_query::Variables {
    fn from(value: GetKeyInput) -> Self {
        get_key_query::Variables {
            key_id: value.key_id,
            organization_id: value.organization_id,
        }
    }
}

pub struct GetKeyResponse {
    pub key: ApiKey,
}

pub async fn run(
    input: GetKeyInput,
    client: &StudioClient,
) -> Result<GetKeyResponse, RoverClientError> {
    let organization_id = input.organization_id.clone();
    let key_id = input.key_id.clone();
    let data = client.post::<GetKeyQuery>(input.into()).await?;
    match data
        .organization
        .ok_or_else(|| OrganizationIDNotFound { organization_id })?
        .api_key
    {
        None => Err(RoverClientError::ApiKeyNotFound { api_key_id: key_id }),
        Some(key) => {
            let key = ApiKey::try_from(key)?;
            Ok(GetKeyResponse { key })
        }
    }
}

impl TryFrom<GetKeyQueryOrganizationApiKey> for ApiKey {
    type Error = RoverClientError;

    fn try_from(value: GetKeyQueryOrganizationApiKey) -> Result<Self, Self::Error> {
        let created_at = DateTime::parse_from_rfc3339(&value.created_at)?;
        let expires_at = match value.expires_at {
            None => None,
            Some(timestamp) => {
                let parsed_timestamp = DateTime::parse_from_rfc3339(&timestamp)?;
                Some(parsed_timestamp)
            }
        };
        Ok(Self {
            created_at,
            expires_at,
            id: value.id.clone(),
            name: value.key_name.clone(),
        })
    }
}

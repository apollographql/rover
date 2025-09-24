use graphql_client::GraphQLQuery;

use crate::blocking::StudioClient;
use crate::operations::api_key::GraphOsKeyType;
use crate::RoverClientError;

#[derive(GraphQLQuery, Debug)]
#[graphql(
    query_path = "src/operations/api_key/create/create_key_mutation.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "Eq, PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
pub(crate) struct CreateKeyMutation;

pub struct CreateKeyInput {
    pub organization_id: String,
    pub name: String,
    pub key_type: GraphOsKeyType,
}

impl From<CreateKeyInput> for create_key_mutation::Variables {
    fn from(value: CreateKeyInput) -> Self {
        create_key_mutation::Variables {
            organization_id: value.organization_id,
            name: value.name,
            type_: value.key_type,
        }
    }
}

pub struct CreateKeyResponse {
    pub key_id: String,
    pub key_name: String,
    pub token: String,
}

pub async fn run(
    input: CreateKeyInput,
    client: &StudioClient,
) -> Result<CreateKeyResponse, RoverClientError> {
    let organization_id = input.organization_id.clone();
    let data = client.post::<CreateKeyMutation>(input.into()).await?;
    build_response(data, organization_id)
}

fn build_response(
    data: create_key_mutation::ResponseData,
    organization_id: String,
) -> Result<CreateKeyResponse, RoverClientError> {
    let key = data
        .organization
        .ok_or_else(|| RoverClientError::OrganizationIDNotFound { organization_id })?
        .create_key;
    // Unwrap below is safe because despite the fact the query response has name as an optional
    // element, the request requires you to specify it, so there's no way it couldn't be returned.
    let key_name = key.key_name.unwrap();
    Ok(CreateKeyResponse {
        key_id: key.id,
        key_name,
        token: key.token,
    })
}

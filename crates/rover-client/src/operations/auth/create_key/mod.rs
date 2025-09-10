use graphql_client::GraphQLQuery;

use crate::blocking::StudioClient;
use crate::operations::auth::create_key::create_key_mutation::GraphOsKeyType;
use crate::RoverClientError;

#[derive(GraphQLQuery, Debug)]
// The paths are relative to the directory where your `Cargo.toml` is located.
// Both json and the GraphQL schema language are supported as sources for the schema
#[graphql(
    query_path = "src/operations/auth/create_key/create_key_mutation.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "Eq, PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
pub struct CreateKeyMutation;

pub(crate) struct CreateKeyInput {
    pub(crate) organization_id: String,
    name: String,
    key_type: GraphOsKeyType,
}

impl From<CreateKeyInput> for create_key_mutation::Variables {
    fn from(value: CreateKeyInput) -> Self {
        create_key_mutation::Variables {
            organization_id: value.organization_id,
            name: value.name,
            type_: value.key_type.into(),
        }
    }
}

pub(crate) struct CreateKeyResponse {
    pub(crate) key_id: String,
    pub(crate) key_name: String,
    pub(crate) token: String,
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

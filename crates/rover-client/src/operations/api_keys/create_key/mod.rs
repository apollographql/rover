use graphql_client::GraphQLQuery;

use crate::blocking::StudioClient;
use crate::operations::api_keys::create_key::create_key_mutation::GraphOsKeyType as QueryGraphOsKeyType;
use crate::RoverClientError;

// We define a new enum here so that we can keep the implementation details of the actual graph
// enum contained within this crate rather than leaking it out. Further it allows us to selectively
// add support for more key types as they are required, rather than them changing as the schema
// does.
#[derive(Debug)]
pub enum GraphOsKeyType {
    OPERATOR,
}

impl GraphOsKeyType {
    fn as_query_enum(&mut self) -> QueryGraphOsKeyType {
        match self {
            Self::OPERATOR => QueryGraphOsKeyType::OPERATOR,
        }
    }
}

#[derive(GraphQLQuery, Debug)]
#[graphql(
    query_path = "src/operations/api_keys/create_key/create_key_mutation.graphql",
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
    fn from(mut value: CreateKeyInput) -> Self {
        create_key_mutation::Variables {
            organization_id: value.organization_id,
            name: value.name,
            type_: value.key_type.as_query_enum(),
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

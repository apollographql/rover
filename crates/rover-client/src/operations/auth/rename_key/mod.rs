use graphql_client::GraphQLQuery;

use crate::blocking::StudioClient;
use crate::RoverClientError;
use crate::RoverClientError::OrganizationIDNotFound;

#[derive(GraphQLQuery, Debug)]
#[graphql(
    query_path = "src/operations/auth/rename_key/rename_key_mutation.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "Eq, PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
pub struct RenameKeyMutation;

pub(crate) struct RenameKeyInput {
    pub(crate) organization_id: String,
    pub(crate) key_id: String,
    pub(crate) new_name: String,
}

impl From<RenameKeyInput> for rename_key_mutation::Variables {
    fn from(value: RenameKeyInput) -> Self {
        rename_key_mutation::Variables {
            key_id: value.key_id,
            name: value.new_name,
            organization_id: value.organization_id,
        }
    }
}

pub(crate) struct RenameKeyResponse {
    pub(crate) key_id: String,
    pub(crate) name: String,
}

pub async fn run(
    input: RenameKeyInput,
    client: &StudioClient,
) -> Result<RenameKeyResponse, RoverClientError> {
    let organization_id = input.organization_id.clone();
    let key_id = input.key_id.clone();
    let data = client.post::<RenameKeyMutation>(input.into()).await?;
    let resp = data
        .organization
        .ok_or_else(|| OrganizationIDNotFound { organization_id })?
        .rename_key;
    // The unwrap below is OK because although name is an optional field in the general case,
    // it's nonsensical for a rename option to not return one.
    Ok(RenameKeyResponse {
        key_id,
        name: resp.key_name.unwrap(),
    })
}

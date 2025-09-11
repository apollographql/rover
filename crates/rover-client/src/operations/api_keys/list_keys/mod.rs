use chrono::{DateTime, FixedOffset};
use graphql_client::GraphQLQuery;
use serde::Serialize;

use crate::blocking::StudioClient;
use crate::operations::api_keys::list_keys::list_keys_query::ListKeysQueryOrganizationApiKeysEdges;
use crate::RoverClientError;
use crate::RoverClientError::OrganizationIDNotFound;

type Timestamp = String;
type RemoteApiKey = ListKeysQueryOrganizationApiKeysEdges;

#[derive(GraphQLQuery, Debug)]
#[graphql(
    query_path = "src/operations/api_keys/list_keys/list_keys_query.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "Eq, PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
struct ListKeysQuery;

#[derive(Clone)]
pub struct ListKeysInput {
    pub organization_id: String,
}

impl From<ListKeysInput> for list_keys_query::Variables {
    fn from(value: ListKeysInput) -> Self {
        list_keys_query::Variables {
            organization_id: value.organization_id,
            after: None,
        }
    }
}

pub struct ListKeysResponse {
    pub keys: Vec<ApiKey>,
}

#[derive(Clone, Eq, PartialEq, Debug, Serialize)]
pub struct ApiKey {
    pub created_at: DateTime<FixedOffset>,
    pub expires_at: Option<DateTime<FixedOffset>>,
    pub id: String,
    pub name: Option<String>,
}

pub async fn run(
    input: ListKeysInput,
    client: &StudioClient,
) -> Result<ListKeysResponse, RoverClientError> {
    let organization_id = input.organization_id.clone();
    // Instantiate the variables outside the loop so we can do pagination properly
    let vars: list_keys_query::Variables = input.clone().into();
    let data = client.post::<ListKeysQuery>(vars).await?;

    // Grab the initial set of API Keys returned
    let api_keys = data
        .organization
        .ok_or_else(|| OrganizationIDNotFound { organization_id })?
        .api_keys;
    let mut final_list = Vec::new();
    final_list.extend(api_keys.edges);

    // Set up pagination variables
    let mut has_next = api_keys.page_info.has_next_page;
    let mut end_cursor = api_keys.page_info.end_cursor;
    while has_next {
        let organization_id = input.organization_id.clone();
        let mut vars: list_keys_query::Variables = input.clone().into();
        vars.after = end_cursor;
        let data = client.post::<ListKeysQuery>(vars).await?;
        let api_keys = data
            .organization
            .ok_or_else(|| OrganizationIDNotFound { organization_id })?
            .api_keys;
        final_list.extend(api_keys.edges);
        has_next = api_keys.page_info.has_next_page;
        end_cursor = api_keys.page_info.end_cursor;
    }

    build_response(final_list)
}

fn build_response(data: Vec<RemoteApiKey>) -> Result<ListKeysResponse, RoverClientError> {
    let mut keys = Vec::new();
    for remote_api_key in data {
        let created_at = DateTime::parse_from_rfc3339(&remote_api_key.node.created_at)?;
        let expires_at = match remote_api_key.node.expires_at {
            None => None,
            Some(timestamp) => {
                let parsed_timestamp = DateTime::parse_from_rfc3339(&timestamp)?;
                Some(parsed_timestamp)
            }
        };
        let new_key = ApiKey {
            created_at,
            expires_at,
            id: remote_api_key.node.id.clone(),
            name: remote_api_key.node.key_name.clone(),
        };
        keys.push(new_key);
    }
    Ok(ListKeysResponse { keys })
}

use chrono::{DateTime, FixedOffset};
use graphql_client::GraphQLQuery;
use serde::Serialize;

use crate::{
    blocking::StudioClient,
    operations::api_key::list::list_keys_query::ListKeysQueryOrganizationApiKeysEdges,
    RoverClientError, RoverClientError::OrganizationIDNotFound,
};

type RemoteApiKey = ListKeysQueryOrganizationApiKeysEdges;
type Timestamp = String;
// `graphql_client` generates its own copy of this enum nested under `list_keys_query` rather
// than reusing `create`'s identically-shaped one (unlike custom scalars, an enum can't be
// pre-aliased into the derive) - this alias is just a readability shorthand for that generated
// type, not a shared one.
type GraphOsKeyType = list_keys_query::GraphOsKeyType;

#[derive(GraphQLQuery, Debug)]
#[graphql(
    query_path = "src/operations/api_key/list/list_keys_query.graphql",
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
    /// `None` when the Platform API doesn't report a type for this key - nullable at the
    /// schema level for any key, not only pairs (spec FR15).
    pub key_type: Option<String>,
}

/// Renders the full backend key-type enum, not just the subset `rover api-key create` can
/// produce (`ApiKeyType`, `src/command/api_key/mod.rs`) - an organization can hold key types
/// Rover has never let anyone create (e.g. `SCIM`, `ROUTER`).
fn render_key_type(key_type: GraphOsKeyType) -> String {
    match key_type {
        GraphOsKeyType::GATEWAY => "Gateway".to_string(),
        GraphOsKeyType::OPERATOR => "Operator".to_string(),
        GraphOsKeyType::ROUTER => "Router".to_string(),
        GraphOsKeyType::SCIM => "Scim".to_string(),
        GraphOsKeyType::SUBGRAPH => "Subgraph".to_string(),
        GraphOsKeyType::VARIANT => "Variant".to_string(),
        // Forward-compatible: a variant this build doesn't know about yet, rather than a panic.
        GraphOsKeyType::Other(other) => other,
    }
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

    let mut keys = Vec::new();
    for remote_api_key in final_list {
        keys.push(remote_api_key.try_into()?);
    }
    Ok(ListKeysResponse { keys })
}

impl TryFrom<RemoteApiKey> for ApiKey {
    type Error = RoverClientError;

    fn try_from(value: RemoteApiKey) -> Result<Self, Self::Error> {
        let created_at = DateTime::parse_from_rfc3339(&value.node.created_at)?;
        let expires_at = match value.node.expires_at {
            None => None,
            Some(timestamp) => {
                let parsed_timestamp = DateTime::parse_from_rfc3339(&timestamp)?;
                Some(parsed_timestamp)
            }
        };
        Ok(Self {
            created_at,
            expires_at,
            id: value.node.id.clone(),
            name: value.node.key_name.clone(),
            key_type: value.node.key_type.map(render_key_type),
        })
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use speculoos::prelude::*;

    use super::*;

    #[rstest]
    #[case::gateway(GraphOsKeyType::GATEWAY, "Gateway")]
    #[case::operator(GraphOsKeyType::OPERATOR, "Operator")]
    #[case::router(GraphOsKeyType::ROUTER, "Router")]
    #[case::scim(GraphOsKeyType::SCIM, "Scim")]
    #[case::subgraph(GraphOsKeyType::SUBGRAPH, "Subgraph")]
    #[case::variant(GraphOsKeyType::VARIANT, "Variant")]
    #[case::forward_compatible_unknown_variant(
        GraphOsKeyType::Other("FUTURE_TYPE".to_string()),
        "FUTURE_TYPE"
    )]
    fn render_key_type_covers_every_known_variant(
        #[case] key_type: GraphOsKeyType,
        #[case] expected: &str,
    ) {
        assert_that!(render_key_type(key_type)).is_equal_to(expected.to_string());
    }

    fn remote_key(key_type: serde_json::Value) -> RemoteApiKey {
        serde_json::from_value(serde_json::json!({
            "node": {
                "createdAt": "2026-01-04T12:00:00Z",
                "expiresAt": null,
                "id": "key-123",
                "keyName": "router-prod",
                "keyType": key_type,
                "token": "s_super-secret",
            }
        }))
        .unwrap()
    }

    #[test]
    fn try_from_renders_a_present_key_type() {
        let key = ApiKey::try_from(remote_key(serde_json::json!("OPERATOR"))).unwrap();

        assert_that!(key.key_type)
            .is_some()
            .is_equal_to("Operator".to_string());
    }

    #[test]
    fn try_from_leaves_a_missing_key_type_as_none() {
        let key = ApiKey::try_from(remote_key(serde_json::Value::Null)).unwrap();

        assert_that!(key.key_type).is_none();
    }
}

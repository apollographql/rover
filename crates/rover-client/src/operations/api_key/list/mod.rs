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
    pub key_type: Option<ApiKeyBackendType>,
}

/// The full set of API key types the Platform API may report for an existing key - a superset
/// of what `rover api-key create` can produce (`ApiKeyType`, `src/command/api_key/mod.rs`): an
/// organization can hold key types Rover has never let anyone create (e.g. `Scim`, `Router`).
/// A real enum a caller can match on exhaustively, rather than a pre-rendered string - extending
/// the set later (e.g. if Rover starts creating `Gateway` keys) only means adding a variant here,
/// not chasing string literals at every comparison site.
#[derive(Clone, Eq, PartialEq, Debug)]
pub enum ApiKeyBackendType {
    Gateway,
    Operator,
    Router,
    Scim,
    Subgraph,
    Variant,
    /// A type this build doesn't know about yet - carries the raw value forward rather than
    /// losing it or panicking.
    Other(String),
}

impl std::fmt::Display for ApiKeyBackendType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Gateway => write!(f, "Gateway"),
            Self::Operator => write!(f, "Operator"),
            Self::Router => write!(f, "Router"),
            Self::Scim => write!(f, "Scim"),
            Self::Subgraph => write!(f, "Subgraph"),
            Self::Variant => write!(f, "Variant"),
            Self::Other(other) => write!(f, "{other}"),
        }
    }
}

/// Serializes as [`Display`](std::fmt::Display) - the exact spelling spec FR15 requires
/// ("Operator", not the raw schema spelling "OPERATOR") - rather than deriving, since no
/// `serde(rename_all = ...)` convention reproduces that from the enum's own Rust variant names
/// once the forward-compatible `Other(String)` variant is in the mix.
impl Serialize for ApiKeyBackendType {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl From<list_keys_query::GraphOsKeyType> for ApiKeyBackendType {
    fn from(key_type: list_keys_query::GraphOsKeyType) -> Self {
        match key_type {
            list_keys_query::GraphOsKeyType::GATEWAY => Self::Gateway,
            list_keys_query::GraphOsKeyType::OPERATOR => Self::Operator,
            list_keys_query::GraphOsKeyType::ROUTER => Self::Router,
            list_keys_query::GraphOsKeyType::SCIM => Self::Scim,
            list_keys_query::GraphOsKeyType::SUBGRAPH => Self::Subgraph,
            list_keys_query::GraphOsKeyType::VARIANT => Self::Variant,
            list_keys_query::GraphOsKeyType::Other(other) => Self::Other(other),
        }
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
            key_type: value.node.key_type.map(ApiKeyBackendType::from),
        })
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use speculoos::prelude::*;

    use super::*;

    #[rstest]
    #[case::gateway(list_keys_query::GraphOsKeyType::GATEWAY, ApiKeyBackendType::Gateway)]
    #[case::operator(list_keys_query::GraphOsKeyType::OPERATOR, ApiKeyBackendType::Operator)]
    #[case::router(list_keys_query::GraphOsKeyType::ROUTER, ApiKeyBackendType::Router)]
    #[case::scim(list_keys_query::GraphOsKeyType::SCIM, ApiKeyBackendType::Scim)]
    #[case::subgraph(list_keys_query::GraphOsKeyType::SUBGRAPH, ApiKeyBackendType::Subgraph)]
    #[case::variant(list_keys_query::GraphOsKeyType::VARIANT, ApiKeyBackendType::Variant)]
    #[case::forward_compatible_unknown_variant(
        list_keys_query::GraphOsKeyType::Other("FUTURE_TYPE".to_string()),
        ApiKeyBackendType::Other("FUTURE_TYPE".to_string())
    )]
    fn from_covers_every_known_variant(
        #[case] key_type: list_keys_query::GraphOsKeyType,
        #[case] expected: ApiKeyBackendType,
    ) {
        assert_that!(ApiKeyBackendType::from(key_type)).is_equal_to(expected);
    }

    #[rstest]
    #[case::gateway(ApiKeyBackendType::Gateway, "Gateway")]
    #[case::operator(ApiKeyBackendType::Operator, "Operator")]
    #[case::router(ApiKeyBackendType::Router, "Router")]
    #[case::scim(ApiKeyBackendType::Scim, "Scim")]
    #[case::subgraph(ApiKeyBackendType::Subgraph, "Subgraph")]
    #[case::variant(ApiKeyBackendType::Variant, "Variant")]
    #[case::forward_compatible_unknown_variant(
        ApiKeyBackendType::Other("FUTURE_TYPE".to_string()),
        "FUTURE_TYPE"
    )]
    fn serializes_as_the_exact_fr15_spelling(
        #[case] key_type: ApiKeyBackendType,
        #[case] expected: &str,
    ) {
        assert_that!(serde_json::to_value(&key_type).unwrap())
            .is_equal_to(serde_json::json!(expected));
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
    fn try_from_converts_a_present_key_type() {
        let key = ApiKey::try_from(remote_key(serde_json::json!("OPERATOR"))).unwrap();

        assert_that!(key.key_type)
            .is_some()
            .is_equal_to(ApiKeyBackendType::Operator);
    }

    #[test]
    fn try_from_leaves_a_missing_key_type_as_none() {
        let key = ApiKey::try_from(remote_key(serde_json::Value::Null)).unwrap();

        assert_that!(key.key_type).is_none();
    }
}

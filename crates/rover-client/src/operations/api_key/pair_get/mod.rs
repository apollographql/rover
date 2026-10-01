use bon::Builder;
use chrono::DateTime;
use graphql_client::GraphQLQuery;

use crate::{
    operations::api_key::pair_list::{OAuthClientPair, PairActor, PairResource},
    RoverClientError,
};

pub mod service;

pub use service::{GetOAuthClient, GET_PAIR_ATTEMPT_TIMEOUT};

type Timestamp = String;

/// Looks up one of an organization's `client_credentials` OAuth clients (client-credential
/// pairs) by its client ID - how `rover api-key delete`/`rename` tell a pair's ID from an API
/// key's (spec FR18-20, `specs/rover-431-identity-grant-management`).
///
/// The Platform API returns `null` here for a client that doesn't exist, one the caller isn't
/// permitted to see, and any caller in an organization not enrolled in client-credential
/// support alike - deliberately, so the response never reveals that a client exists. A `null`
/// therefore means "not a pair, or Rover can't tell", never specifically "not a pair": exactly
/// the case FR20 says must fall back to treating the ID as an API key.
#[derive(GraphQLQuery, Debug)]
#[graphql(
    query_path = "src/operations/api_key/pair_get/get_pair_query.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "Eq, PartialEq, Debug, Serialize, Deserialize",
    variables_derives = "Clone, PartialEq, Debug",
    deprecated = "warn"
)]
pub struct GetPairQuery;

#[derive(Clone, Debug, Builder)]
pub struct GetOAuthClientInput {
    #[builder(into)]
    pub organization_id: String,
    #[builder(into)]
    pub client_id: String,
}

type RemoteOAuthClient = get_pair_query::GetPairQueryOrganizationOauthClient;
type RemoteActor = get_pair_query::GetPairQueryOrganizationOauthClientCreatedBy;
type RemoteResource = get_pair_query::GetPairQueryOrganizationOauthClientResources;

/// Renders this query's `ActorType` the way `pair_list`'s hand-written `Display` renders its own
/// (spec FR15): the serialized schema spelling, lowercased - `SERVICE_ACCOUNT` becomes
/// `service_account`, and an unknown variant's raw value is lowercased the same way. The two
/// agree today because lowercasing the schema spelling is exactly what `pair_list`'s match does,
/// but nothing ties them together; the tests below pin this side's output.
fn actor_kind(actor_type: &get_pair_query::ActorType) -> String {
    serde_json::to_value(actor_type)
        .ok()
        .and_then(|value| value.as_str().map(str::to_lowercase))
        .unwrap_or_default()
}

impl TryFrom<RemoteOAuthClient> for OAuthClientPair {
    type Error = RoverClientError;

    fn try_from(value: RemoteOAuthClient) -> Result<Self, Self::Error> {
        Ok(Self {
            client_id: value.client_id,
            name: value.client_name,
            created_at: DateTime::parse_from_rfc3339(&value.created_at)?,
            created_by: value.created_by.into(),
            resources: value.resources.into_iter().map(Into::into).collect(),
            scopes: value.scopes,
        })
    }
}

impl From<RemoteActor> for PairActor {
    fn from(value: RemoteActor) -> Self {
        Self {
            kind: actor_kind(&value.type_),
            id: value.actor_id,
        }
    }
}

impl From<RemoteResource> for PairResource {
    fn from(value: RemoteResource) -> Self {
        Self {
            resource_id: value.resource_id,
            resource_type: value.resource_type,
        }
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use speculoos::prelude::*;

    use super::*;

    #[rstest]
    #[case::user(get_pair_query::ActorType::USER, "user")]
    #[case::service_account(get_pair_query::ActorType::SERVICE_ACCOUNT, "service_account")]
    #[case::forward_compatible_unknown_variant(
        get_pair_query::ActorType::Other("FUTURE_KIND".to_string()),
        "future_kind"
    )]
    fn actor_kind_matches_pair_lists_lowercased_rendering(
        #[case] actor_type: get_pair_query::ActorType,
        #[case] expected: &str,
    ) {
        assert_that!(actor_kind(&actor_type)).is_equal_to(expected.to_string());
    }
}

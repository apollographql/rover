use bon::Builder;
use chrono::{DateTime, FixedOffset};
use graphql_client::GraphQLQuery;
use serde::Serialize;

use crate::RoverClientError;

pub mod service;

pub use service::{ListOAuthClients, ListOAuthClientsError};

type Timestamp = String;

/// How many pairs [`ListOAuthClients`](service::ListOAuthClients) collects per call when
/// [`ListOAuthClientsInput::limit`] isn't overridden. A caller that needs the complete set
/// regardless of size (e.g. the per-user sweep's enumeration, spec FR56) must page explicitly
/// via [`ListOAuthClientsResponse::next_after`] rather than raise this without bound.
pub const DEFAULT_PAIR_LIST_LIMIT: usize = 50;

/// Lists an organization's `client_credentials` OAuth clients (client-credential pairs), one page
/// at a time, up to [`ListOAuthClientsInput::limit`] per call. Shared by `rover api-key list`
/// (PRD A1.2) and the per-user grant sweep's enumeration step (PRD B3.3, FR56) — see
/// `specs/rover-431-identity-grant-management`.
#[derive(GraphQLQuery, Debug)]
#[graphql(
    query_path = "src/operations/api_key/pair_list/list_pairs_query.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "Eq, PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
pub struct ListPairsQuery;

#[derive(Clone, Debug, Builder)]
pub struct ListOAuthClientsInput {
    #[builder(into)]
    pub organization_id: String,
    /// Resume from this cursor (a previous call's [`ListOAuthClientsResponse::next_after`]).
    /// `None` starts from the first page.
    #[builder(into)]
    pub after: Option<String>,
    /// Collect at most this many pairs before returning, across as many pages as it takes.
    /// See [`DEFAULT_PAIR_LIST_LIMIT`].
    #[builder(default = DEFAULT_PAIR_LIST_LIMIT)]
    pub limit: usize,
}

/// The result of one [`ListOAuthClients`] call: the pairs it collected, plus where to resume if
/// the organization holds more than `limit`.
#[derive(Clone, PartialEq, Debug)]
pub struct ListOAuthClientsResponse {
    pub pairs: Vec<OAuthClientPair>,
    /// `Some` when more pairs exist beyond `limit` — pass this back as the next call's `after`
    /// to continue. `None` means every remaining pair was returned.
    pub next_after: Option<String>,
}

/// One `client_credentials` OAuth client (client-credential pair), as reported by
/// `Organization.oauthClients`. Secrets are never requested by this operation.
#[derive(Clone, PartialEq, Debug, Serialize)]
pub struct OAuthClientPair {
    pub client_id: String,
    pub name: Option<String>,
    pub created_at: DateTime<FixedOffset>,
    pub created_by: PairActor,
    pub resources: Vec<PairResource>,
    pub scopes: Vec<String>,
}

/// The actor that registered a pair. `kind` is Rover's lowercased rendering of the Platform
/// API's `ActorType` enum (`"user"`, `"service_account"`, ...) — see spec FR15.
#[derive(Clone, PartialEq, Debug, Serialize)]
pub struct PairActor {
    pub id: String,
    pub kind: String,
}

/// A resource a pair is restricted to. `resource_type` is a plain string at the source (e.g.
/// `"GRAPH"`); this operation does not filter or interpret it — a caller that only cares about
/// graphs (e.g. FR12's `graphs` list) filters for that type itself.
#[derive(Clone, PartialEq, Debug, Serialize)]
pub struct PairResource {
    pub resource_id: String,
    pub resource_type: String,
}

type RemoteOAuthClient = list_pairs_query::ListPairsQueryOrganizationOauthClientsEdgesNode;
type RemoteActor = list_pairs_query::ListPairsQueryOrganizationOauthClientsEdgesNodeCreatedBy;
type RemoteResource = list_pairs_query::ListPairsQueryOrganizationOauthClientsEdgesNodeResources;
type RemoteActorType = list_pairs_query::ActorType;

impl TryFrom<RemoteOAuthClient> for OAuthClientPair {
    type Error = RoverClientError;

    fn try_from(value: RemoteOAuthClient) -> Result<Self, Self::Error> {
        let created_at = DateTime::parse_from_rfc3339(&value.created_at)?;
        Ok(Self {
            client_id: value.client_id,
            name: value.client_name,
            created_at,
            created_by: value.created_by.into(),
            resources: value.resources.into_iter().map(Into::into).collect(),
            scopes: value.scopes,
        })
    }
}

impl From<RemoteActor> for PairActor {
    fn from(value: RemoteActor) -> Self {
        Self {
            id: value.actor_id,
            kind: actor_kind(value.type_),
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

fn actor_kind(actor_type: RemoteActorType) -> String {
    match actor_type {
        RemoteActorType::ANONYMOUS_USER => "anonymous_user",
        RemoteActorType::BACKFILL => "backfill",
        RemoteActorType::CRON => "cron",
        RemoteActorType::GRAPH => "graph",
        RemoteActorType::INTERNAL_IDENTITY => "internal_identity",
        RemoteActorType::SERVICE_ACCOUNT => "service_account",
        RemoteActorType::SYNCHRONIZATION => "synchronization",
        RemoteActorType::SYSTEM => "system",
        RemoteActorType::USER => "user",
        // Forward-compatible: a variant this build doesn't know about yet, rather than a panic.
        RemoteActorType::Other(ref other) => return other.to_lowercase(),
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use speculoos::prelude::*;

    use super::*;

    #[rstest]
    #[case::anonymous_user(RemoteActorType::ANONYMOUS_USER, "anonymous_user")]
    #[case::backfill(RemoteActorType::BACKFILL, "backfill")]
    #[case::cron(RemoteActorType::CRON, "cron")]
    #[case::graph(RemoteActorType::GRAPH, "graph")]
    #[case::internal_identity(RemoteActorType::INTERNAL_IDENTITY, "internal_identity")]
    #[case::service_account(RemoteActorType::SERVICE_ACCOUNT, "service_account")]
    #[case::synchronization(RemoteActorType::SYNCHRONIZATION, "synchronization")]
    #[case::system(RemoteActorType::SYSTEM, "system")]
    #[case::user(RemoteActorType::USER, "user")]
    #[case::forward_compatible_unknown_variant(
        RemoteActorType::Other("FUTURE_KIND".to_string()),
        "future_kind"
    )]
    fn actor_kind_lowercases_every_known_variant_and_falls_back_for_unknown_ones(
        #[case] actor_type: RemoteActorType,
        #[case] expected: &str,
    ) {
        assert_that!(actor_kind(actor_type)).is_equal_to(expected.to_string());
    }
}

use bon::Builder;
use chrono::{DateTime, FixedOffset};
use graphql_client::GraphQLQuery;
use serde::Serialize;

use crate::{
    operations::api_key::{missing_secret_data, pair_list::PairResource},
    RoverClientError,
};

pub mod service;

pub use service::CreatePair;

type Timestamp = String;

/// Creates a `client_credentials` OAuth client (client-credential pair) under an organization.
/// Always requests exactly the `rover:cli` scope, hardcoded in the mutation itself rather than
/// on [`CreatePairInput`] - spec FR5 requires Rover to expose no flag that selects a scope or a
/// role, so there's no input field to misuse. See `specs/rover-431-identity-grant-management`.
#[derive(GraphQLQuery, Debug)]
#[graphql(
    query_path = "src/operations/api_key/pair_create/create_pair_mutation.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "Eq, PartialEq, Debug, Serialize, Deserialize",
    variables_derives = "Clone, PartialEq, Debug",
    deprecated = "warn"
)]
pub struct CreatePairMutation;

#[derive(Clone, Debug, Builder)]
pub struct CreatePairInput {
    #[builder(into)]
    pub organization_id: String,
    #[builder(into)]
    pub name: String,
    /// The graphs to restrict the pair to (FR2). Not deduplicated here - a caller that collects
    /// repeated `--graph-id` values is responsible for that before building this input.
    pub graph_ids: Vec<String>,
    /// `None` uses the Platform API's default secret lifetime.
    #[builder(into)]
    pub secret_lifetime_days: Option<i64>,
}

/// A newly created pair's secret is shown exactly once, in this response - see spec FR6/FR7.
#[derive(Clone, PartialEq, Debug, Serialize)]
pub struct CreatedPair {
    pub client_id: String,
    pub name: Option<String>,
    pub client_secret: String,
    pub secret_expires_at: DateTime<FixedOffset>,
    pub resources: Vec<PairResource>,
    pub scopes: Vec<String>,
}

type RemoteCreatedPair = create_pair_mutation::CreatePairMutationOrganizationCreateOAuthClient;
type RemoteResource =
    create_pair_mutation::CreatePairMutationOrganizationCreateOAuthClientResources;

impl TryFrom<RemoteCreatedPair> for CreatedPair {
    type Error = RoverClientError;

    fn try_from(value: RemoteCreatedPair) -> Result<Self, Self::Error> {
        let client_secret = value.client_secret.ok_or_else(missing_secret_data)?;
        let secret_expires_at = value.secret_expires_at.ok_or_else(missing_secret_data)?;
        Ok(Self {
            client_id: value.client_id,
            name: value.client_name,
            client_secret,
            secret_expires_at: DateTime::parse_from_rfc3339(&secret_expires_at)?,
            resources: value.resources.into_iter().map(Into::into).collect(),
            scopes: value.scopes,
        })
    }
}

// `PairResource` is defined in the sibling `pair_list` module (it's the same underlying
// `OAuthClientResource` schema type) and reused here rather than duplicated.
impl From<RemoteResource> for PairResource {
    fn from(value: RemoteResource) -> Self {
        Self {
            resource_id: value.resource_id,
            resource_type: value.resource_type,
        }
    }
}

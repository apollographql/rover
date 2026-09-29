use bon::Builder;
use chrono::{DateTime, FixedOffset};
use graphql_client::GraphQLQuery;
use serde::Serialize;

use crate::RoverClientError;

pub mod service;

pub use service::RotatePair;

type Timestamp = String;

/// Rotates a `client_credentials` OAuth client's (client-credential pair's) secret: mints a new
/// one and schedules every other secret the pair holds to stop working after the grace period
/// (spec FR21-27).
#[derive(GraphQLQuery, Debug)]
#[graphql(
    query_path = "src/operations/api_key/pair_rotate/rotate_pair_mutation.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "Eq, PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
pub struct RotatePairMutation;

#[derive(Clone, Debug, Builder)]
pub struct RotatePairInput {
    #[builder(into)]
    pub organization_id: String,
    #[builder(into)]
    pub client_id: String,
    /// `None` means an immediate cutover (the Platform API's own default) - Rover must not
    /// substitute a default of its own (spec FR22).
    #[builder(into)]
    pub grace_period_days: Option<i64>,
}

/// A pair's freshly rotated secret, shown exactly once (spec FR24). Name and graphs are
/// unchanged by rotation (FR24) and so aren't part of this response at all.
#[derive(Clone, PartialEq, Debug, Serialize)]
pub struct RotatedPair {
    pub client_id: String,
    pub client_secret: String,
    pub secret_expires_at: DateTime<FixedOffset>,
}

type RemoteRotatedPair =
    rotate_pair_mutation::RotatePairMutationOrganizationRotateOAuthClientSecret;

impl TryFrom<RemoteRotatedPair> for RotatedPair {
    type Error = RoverClientError;

    fn try_from(value: RemoteRotatedPair) -> Result<Self, Self::Error> {
        let client_secret = value.client_secret.ok_or_else(missing_secret_data)?;
        let secret_expires_at = value.secret_expires_at.ok_or_else(missing_secret_data)?;
        Ok(Self {
            client_id: value.client_id,
            client_secret,
            secret_expires_at: DateTime::parse_from_rfc3339(&secret_expires_at)?,
        })
    }
}

/// `client_secret`/`secret_expires_at` are nullable at the schema level (non-null only
/// alongside a freshly minted secret, per `OAuthClient`'s doc comment) but always present on a
/// successful create/rotate response - their absence here means something is actually wrong,
/// not a legitimate empty state.
fn missing_secret_data() -> RoverClientError {
    RoverClientError::ClientError {
        msg: "the Platform API did not return the pair's new secret and its expiry".to_string(),
    }
}

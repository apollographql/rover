use bon::Builder;
use chrono::{DateTime, FixedOffset, Utc};
use graphql_client::GraphQLQuery;
use serde::Serialize;

use crate::{operations::api_key::missing_secret_data, RoverClientError};

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
    variables_derives = "Clone, PartialEq, Debug",
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

/// A pair's freshly rotated secret, shown exactly once (spec FR24). Graphs are unchanged by
/// rotation (FR24) and so aren't part of this response at all; `name` is included only because
/// spec FR25's required stderr text names the pair.
#[derive(Clone, PartialEq, Debug, Serialize)]
pub struct RotatedPair {
    pub client_id: String,
    pub name: Option<String>,
    pub client_secret: String,
    pub secret_expires_at: DateTime<FixedOffset>,
    /// When every secret but the new one stops working (spec FR25, FR26) - the moment of
    /// rotation itself when the grace period is zero, never `null` either way (see spec.md's
    /// "Decisions" section on why this is never absent). Not a field the Platform API reports -
    /// `rotateOAuthClientSecret`'s response has no such field - so this is computed from the
    /// `now` this operation's caller captured and the `grace_period_days` it requested, not
    /// parsed off the response.
    pub previous_secrets_expire_at: DateTime<FixedOffset>,
}

type RemoteRotatedPair =
    rotate_pair_mutation::RotatePairMutationOrganizationRotateOAuthClientSecret;

/// Builds a [`RotatedPair`] from the mutation's response, `now` (captured by the caller right
/// after the mutation succeeds, as close to the actual rotation as this process can observe),
/// and the `grace_period_days` that was requested (`None`/`Some(0)` both mean an immediate
/// cutover - spec FR22).
pub(crate) fn rotated_pair_from(
    value: RemoteRotatedPair,
    now: DateTime<Utc>,
    grace_period_days: Option<i64>,
) -> Result<RotatedPair, RoverClientError> {
    let client_secret = value.client_secret.ok_or_else(missing_secret_data)?;
    let secret_expires_at = value.secret_expires_at.ok_or_else(missing_secret_data)?;
    let previous_secrets_expire_at = match grace_period_days {
        Some(days) if days > 0 => now + chrono::Duration::days(days),
        _ => now,
    };
    Ok(RotatedPair {
        client_id: value.client_id,
        name: value.client_name,
        client_secret,
        secret_expires_at: DateTime::parse_from_rfc3339(&secret_expires_at)?,
        previous_secrets_expire_at: previous_secrets_expire_at.into(),
    })
}

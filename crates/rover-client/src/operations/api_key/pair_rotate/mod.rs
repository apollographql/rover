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

/// `--grace-period-days` only validates non-negative (FR23 leaves the upper bound to the
/// Platform API), so a value past what a `DateTime` can represent reaches here - `chrono::
/// Duration::days`/`DateTime + TimeDelta` both panic on overflow, which would be the worst
/// possible failure mode this far along: the secret has already rotated server-side, so a
/// panic here loses it instead of reporting it (the same FR71 concern this operation otherwise
/// protects against for a *timed-out* rotation).
fn grace_period_too_large(days: i64) -> RoverClientError {
    RoverClientError::ClientError {
        msg: format!(
            "the requested grace period ({days} days) is too large to represent as a date"
        ),
    }
}

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
        Some(days) if days > 0 => {
            let delta =
                chrono::TimeDelta::try_days(days).ok_or_else(|| grace_period_too_large(days))?;
            now.checked_add_signed(delta)
                .ok_or_else(|| grace_period_too_large(days))?
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn remote_pair() -> RemoteRotatedPair {
        RemoteRotatedPair {
            client_id: "c_8f2a".to_string(),
            client_name: Some("ci-deploy".to_string()),
            client_secret: Some("s_new-secret".to_string()),
            secret_expires_at: Some("2028-09-25T16:00:00Z".to_string()),
        }
    }

    // A grace period past what `TimeDelta`/`DateTime` can represent must fail cleanly, not
    // panic - the secret has already rotated server-side by the time this runs, so a panic
    // here would lose it instead of reporting it.
    #[test]
    fn an_overflowing_grace_period_fails_instead_of_panicking() {
        let now = Utc::now();

        let result = rotated_pair_from(remote_pair(), now, Some(i64::MAX));

        assert!(result.is_err());
    }

    #[test]
    fn a_normal_grace_period_still_computes_the_expected_expiry() {
        let now = DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);

        let pair = rotated_pair_from(remote_pair(), now, Some(30)).unwrap();

        assert_eq!(
            pair.previous_secrets_expire_at,
            DateTime::parse_from_rfc3339("2026-01-31T00:00:00Z").unwrap()
        );
    }
}

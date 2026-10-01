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

/// Computes FR25/FR26's `previous_secrets_expire_at` from `now` and the requested
/// `grace_period_days` (`None`/`Some(0)` both mean an immediate cutover - spec FR22). Called
/// *before* `RotatePair::call` sends the mutation, deliberately: `--grace-period-days` only
/// validates non-negative at parse time (FR23 leaves the upper bound to the Platform API), so a
/// value past what `chrono` can represent reaches here, and failing before the request is sent
/// means nothing has changed yet. Doing this same check *after* a successful mutation would mean
/// losing an already-rotated secret that can never be shown again (FR71) - the worst possible
/// place for this to fail.
pub(crate) fn previous_secrets_expire_at(
    now: DateTime<Utc>,
    grace_period_days: Option<i64>,
) -> Result<DateTime<FixedOffset>, RoverClientError> {
    let expiry = match grace_period_days {
        Some(days) if days > 0 => {
            let too_large = || RoverClientError::GracePeriodTooLarge { days };
            let delta = chrono::TimeDelta::try_days(days).ok_or_else(too_large)?;
            now.checked_add_signed(delta).ok_or_else(too_large)?
        }
        _ => now,
    };
    Ok(expiry.into())
}

/// Builds a [`RotatedPair`] from the mutation's response and the already-computed
/// `previous_secrets_expire_at` (see [`previous_secrets_expire_at`], computed before the
/// mutation was ever sent).
pub(crate) fn rotated_pair_from(
    value: RemoteRotatedPair,
    previous_secrets_expire_at: DateTime<FixedOffset>,
) -> Result<RotatedPair, RoverClientError> {
    let client_secret = value.client_secret.ok_or_else(missing_secret_data)?;
    let secret_expires_at = value.secret_expires_at.ok_or_else(missing_secret_data)?;
    Ok(RotatedPair {
        client_id: value.client_id,
        name: value.client_name,
        client_secret,
        secret_expires_at: DateTime::parse_from_rfc3339(&secret_expires_at)?,
        previous_secrets_expire_at,
    })
}

#[cfg(test)]
mod tests {
    use speculoos::prelude::*;

    use super::*;

    fn remote_pair() -> RemoteRotatedPair {
        RemoteRotatedPair {
            client_id: "c_8f2a".to_string(),
            client_name: Some("ci-deploy".to_string()),
            client_secret: Some("s_new-secret".to_string()),
            secret_expires_at: Some("2028-09-25T16:00:00Z".to_string()),
        }
    }

    fn now() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    // A grace period past what `TimeDelta` itself can represent must fail cleanly, not panic -
    // and, critically, this runs before `RotatePair::call` ever sends the mutation (see this
    // function's own doc comment), so failing here can never lose an already-rotated secret.
    #[test]
    fn a_grace_period_too_large_for_time_delta_is_rejected() {
        let result = previous_secrets_expire_at(now(), Some(i64::MAX));

        let error = result.expect_err("expected an out-of-range grace period to be rejected");
        assert_that!(error).matches(|error| {
            matches!(error, RoverClientError::GracePeriodTooLarge { days } if *days == i64::MAX)
        });
    }

    // A grace period that fits within `TimeDelta` itself, but which - once added to `now` -
    // falls outside what `DateTime` can represent: exercises `checked_add_signed`'s own failure
    // path specifically, distinct from `try_days`'s (the only path the test above exercises).
    #[test]
    fn a_grace_period_too_large_to_add_to_now_is_rejected() {
        let days = 200_000_000;

        let result = previous_secrets_expire_at(now(), Some(days));

        let error = result.expect_err("expected an out-of-range grace period to be rejected");
        assert_that!(error).matches(|error| {
            matches!(error, RoverClientError::GracePeriodTooLarge { days: d } if *d == days)
        });
    }

    #[test]
    fn a_normal_grace_period_computes_the_expected_expiry() {
        let expiry = previous_secrets_expire_at(now(), Some(30)).unwrap();

        assert_that!(expiry)
            .is_equal_to(DateTime::parse_from_rfc3339("2026-01-31T00:00:00Z").unwrap());
    }

    #[test]
    fn a_zero_grace_period_returns_now_unchanged() {
        let expiry = previous_secrets_expire_at(now(), Some(0)).unwrap();

        assert_that!(expiry)
            .is_equal_to(DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z").unwrap());
    }

    #[test]
    fn an_omitted_grace_period_returns_now_unchanged() {
        let expiry = previous_secrets_expire_at(now(), None).unwrap();

        assert_that!(expiry)
            .is_equal_to(DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z").unwrap());
    }

    #[test]
    fn rotated_pair_from_maps_every_field() {
        let previous_secrets_expire_at =
            DateTime::parse_from_rfc3339("2026-01-31T00:00:00Z").unwrap();

        let pair = rotated_pair_from(remote_pair(), previous_secrets_expire_at).unwrap();

        assert_that!(pair).is_equal_to(RotatedPair {
            client_id: "c_8f2a".to_string(),
            name: Some("ci-deploy".to_string()),
            client_secret: "s_new-secret".to_string(),
            secret_expires_at: DateTime::parse_from_rfc3339("2028-09-25T16:00:00Z").unwrap(),
            previous_secrets_expire_at,
        });
    }
}

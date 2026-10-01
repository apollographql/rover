mod output;

use clap::Parser;
use output::RotateOutput;
use rover_client::{
    RoverClientError,
    operations::api_key::pair_rotate::{
        RotatePair, RotatePairInput, service::ROTATE_PAIR_ATTEMPT_TIMEOUT,
    },
};
use serde::Serialize;
use tower::{Service, ServiceExt};

use crate::{
    RoverError, RoverErrorSuggestion, RoverOutput, RoverResult, command::api_key::OrganizationOpt,
    options::ProfileOpt, utils::client::StudioClientConfig,
};

#[derive(Debug, Serialize, Parser)]
pub(crate) struct Rotate {
    #[clap(flatten)]
    organization_opt: OrganizationOpt,

    #[clap(help = "The client ID of the client-credential pair to rotate")]
    client_id: String,

    /// FR22: `None` means an immediate cutover (the Platform API's own default) - Rover must
    /// never substitute a default of its own. FR23: a usage error if given but not a
    /// non-negative whole number; the upper bound is the Platform API's to enforce.
    #[clap(
        long,
        value_parser = clap::value_parser!(i64).range(0..),
        help = "How many days the pair's previous secrets keep working (default: 0, immediate)"
    )]
    grace_period_days: Option<i64>,
}

impl Rotate {
    pub(crate) async fn run(
        &self,
        client_config: StudioClientConfig,
        profile: &ProfileOpt,
    ) -> RoverResult<RoverOutput> {
        let client = client_config.get_authenticated_client(profile)?;
        let service = client.studio_graphql_service_with_timeout(ROTATE_PAIR_ATTEMPT_TIMEOUT)?;
        let mut rotate_pair = RotatePair::new(service);
        let rotate_pair = rotate_pair.ready().await?;
        let input = RotatePairInput::builder()
            .organization_id(self.organization_opt.organization_id.clone())
            .client_id(self.client_id.clone())
            .maybe_grace_period_days(self.grace_period_days)
            .build();

        let pair = match rotate_pair.call(input).await {
            Ok(pair) => pair,
            // All three mean nothing was rotated - safe to report as an ordinary failure.
            Err(
                err @ (RoverClientError::PairPermissionDenied { .. }
                | RoverClientError::PairNotFound { .. }
                | RoverClientError::OrganizationIDNotFound { .. }),
            ) => return Err(err.into()),
            // Everything else (a timeout, a 5xx, a malformed response) is genuinely ambiguous:
            // `pair_rotate::service`'s own doc comment on `RotatePair` requires the consumer to
            // say so rather than reporting a plain failure, since the mutation may have already
            // committed server-side before the client gave up waiting (FR71: never print a
            // secret on an uncertain outcome).
            Err(err) => {
                return Err(RoverError::new(err).with_suggestion(RoverErrorSuggestion::Adhoc(
                    format!(
                        "Whether the secret was rotated is unknown - check with `rover api-key list {}`.",
                        self.organization_opt.organization_id
                    ),
                )));
            }
        };

        Ok(RoverOutput::CliOutput(Box::new(RotateOutput {
            pair,
            grace_period_days: self.grace_period_days.unwrap_or(0),
        })))
    }
}

#[cfg(test)]
mod tests {
    use speculoos::prelude::*;

    use super::*;

    #[test]
    fn positional_organization_and_client_id_parse() {
        let rotate = Rotate::try_parse_from(["api-key rotate", "acme", "c_8f2a"])
            .expect("expected <ORGANIZATION_ID> <CLIENT_ID> to parse");

        assert_that!(rotate.organization_opt.organization_id).is_equal_to("acme".to_string());
        assert_that!(rotate.client_id).is_equal_to("c_8f2a".to_string());
        assert_that!(rotate.grace_period_days).is_none();
    }

    #[test]
    fn grace_period_days_parses_through() {
        let rotate = Rotate::try_parse_from([
            "api-key rotate",
            "acme",
            "c_8f2a",
            "--grace-period-days",
            "7",
        ])
        .expect("expected --grace-period-days to parse");

        assert_that!(rotate.grace_period_days)
            .is_some()
            .is_equal_to(7);
    }

    // FR23: a usage error, not a request - rejected at parse time.
    #[test]
    fn a_negative_grace_period_is_rejected_by_clap() {
        let error =
            Rotate::try_parse_from(["api-key rotate", "acme", "c_8f2a", "--grace-period-days=-1"])
                .expect_err("expected a negative grace period to be rejected");

        assert_that!(error.to_string()).contains("grace-period-days");
    }

    #[test]
    fn zero_grace_period_is_accepted_by_clap() {
        let rotate = Rotate::try_parse_from([
            "api-key rotate",
            "acme",
            "c_8f2a",
            "--grace-period-days",
            "0",
        ])
        .expect("expected --grace-period-days 0 to parse");

        assert_that!(rotate.grace_period_days)
            .is_some()
            .is_equal_to(0);
    }
}

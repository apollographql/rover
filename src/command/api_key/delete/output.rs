use rover_client::{RoverClientError, operations::api_key::pair_list::OAuthClientPair};

use crate::{RoverError, RoverErrorSuggestion, RoverResult, command::CliOutput};

/// FR28-FR30 (`specs/rover-431-identity-grant-management`): the result of deleting a
/// client-credential pair. Like deleting an API key, there's nothing on stdout - the
/// confirmation goes to stderr (`stderr()`), which for a pair also has to say that already
/// issued tokens outlive it (FR29).
#[derive(Debug, PartialEq)]
pub(crate) struct DeletePairOutput {
    pub(crate) pair: OAuthClientPair,
}

impl DeletePairOutput {
    /// Builds the output from the delete mutation's result, or the error to report instead.
    /// Kept synchronous and separate from the request so every branch is directly unit-testable.
    pub(crate) fn new(
        organization_id: &str,
        pair: OAuthClientPair,
        result: Result<(), RoverClientError>,
    ) -> RoverResult<Self> {
        match result {
            Ok(()) => Ok(Self { pair }),
            // Both mean nothing was deleted - safe to report as an ordinary failure.
            Err(
                err @ (RoverClientError::PairPermissionDenied { .. }
                | RoverClientError::OrganizationIDNotFound { .. }),
            ) => Err(err.into()),
            // Everything else (a timeout, a 5xx, a malformed response) is genuinely ambiguous:
            // `pair_delete::service`'s own doc comment on `DeletePair` requires the consumer to
            // say so rather than reporting a plain failure, since the mutation may have already
            // committed server-side before the client gave up waiting.
            Err(err) => Err(
                RoverError::new(err).with_suggestion(RoverErrorSuggestion::Adhoc(format!(
                    "Whether the pair was deleted is unknown - check with `rover api-key list {organization_id}`."
                ))),
            ),
        }
    }
}

impl CliOutput for DeletePairOutput {
    fn text(&self) -> String {
        String::new()
    }

    fn stderr(&self) -> Option<String> {
        // FR29 names the pair; fall back to its client ID when it has no name, as rotate does.
        let name = self.pair.name.as_deref().unwrap_or(&self.pair.client_id);
        Some(format!(
            "Deleted client-credential pair `{name}` (`{}`). It can no longer obtain tokens. \
            Access tokens it already holds keep working until they expire, for up to 15 minutes.",
            self.pair.client_id
        ))
    }

    /// FR30: keeps the `id` field an API-key delete already reports, and adds `key_type` - known
    /// here without any extra call, since the lookup already established this is a pair.
    fn json(&self) -> Result<serde_json::Value, serde_json::Error> {
        Ok(serde_json::json!({
            "id": self.pair.client_id,
            "key_type": "ClientCredentials",
        }))
    }
}

#[cfg(test)]
mod tests {
    use speculoos::prelude::*;

    use super::*;
    use crate::{RoverOutput, command::api_key::pair_lookup::test_pair, options::JsonOutput};

    #[test]
    fn a_successful_delete_reports_the_pair() {
        let output = DeletePairOutput::new("acme", test_pair(), Ok(()));

        assert_that!(output)
            .is_ok()
            .is_equal_to(DeletePairOutput { pair: test_pair() });
    }

    // Nothing was deleted, so no "outcome unknown" suggestion - just the stable E053 error.
    #[test]
    fn a_permission_denial_is_an_ordinary_failure() {
        let err = DeletePairOutput::new(
            "acme",
            test_pair(),
            Err(RoverClientError::PairPermissionDenied {
                organization_id: "acme".to_string(),
            }),
        )
        .expect_err("expected a permission denial to fail");

        assert_that!(err.code().map(|code| code.to_string()))
            .is_some()
            .is_equal_to("E053".to_string());
        assert_that!(
            err.suggestions()
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
        )
        .is_equal_to(Vec::<String>::new());
    }

    // A timeout or 5xx may have happened after the delete committed - say the outcome is unknown.
    #[test]
    fn an_ambiguous_failure_says_the_outcome_is_unknown() {
        let err = DeletePairOutput::new(
            "acme",
            test_pair(),
            Err(RoverClientError::ClientError {
                msg: "timed out".to_string(),
            }),
        )
        .expect_err("expected an ambiguous failure to fail");

        assert_that!(
            err.suggestions()
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
        )
        .is_equal_to(vec![
            "Whether the pair was deleted is unknown - check with `rover api-key list acme`."
                .to_string(),
        ]);
    }

    #[test]
    fn stderr_carries_fr29s_exact_text() {
        let output = DeletePairOutput { pair: test_pair() };

        assert_that!(output.stderr()).is_some().is_equal_to(
            "Deleted client-credential pair `ci-deploy` (`c_8f2a`). It can no longer obtain \
            tokens. Access tokens it already holds keep working until they expire, for up to 15 \
            minutes."
                .to_string(),
        );
    }

    #[test]
    fn stderr_falls_back_to_the_client_id_when_the_pair_has_no_name() {
        let output = DeletePairOutput {
            pair: OAuthClientPair {
                name: None,
                ..test_pair()
            },
        };

        assert_that!(output.stderr()).is_some().is_equal_to(
            "Deleted client-credential pair `c_8f2a` (`c_8f2a`). It can no longer obtain \
            tokens. Access tokens it already holds keep working until they expire, for up to 15 \
            minutes."
                .to_string(),
        );
    }

    // Matches deleting an API key: nothing on stdout.
    #[test]
    fn text_is_empty() {
        assert_that!(DeletePairOutput { pair: test_pair() }.text()).is_equal_to(String::new());
    }

    #[test]
    fn full_envelope_snapshot() {
        let output = RoverOutput::CliOutput(Box::new(DeletePairOutput { pair: test_pair() }));

        insta::assert_json_snapshot!(JsonOutput::from(&output));
    }
}

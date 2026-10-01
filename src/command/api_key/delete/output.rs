use rover_client::operations::api_key::pair_list::OAuthClientPair;

use crate::command::CliOutput;

/// FR28-FR30 (`specs/rover-431-identity-grant-management`): the result of deleting a
/// client-credential pair. Like deleting an API key, there's nothing on stdout - the
/// confirmation goes to stderr (`stderr()`), which for a pair also has to say that already
/// issued tokens outlive it (FR29).
#[derive(Debug)]
pub(crate) struct DeletePairOutput {
    pub(crate) pair: OAuthClientPair,
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

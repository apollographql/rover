use chrono::SecondsFormat;
use rover_client::operations::api_key::pair_rotate::RotatedPair;
use rover_std::Style;

use crate::{command::CliOutput, utils::table};

/// FR21-FR26 (`specs/rover-431-identity-grant-management`): the result of rotating a
/// client-credential pair's secret. The new secret is shown exactly once, here - `stderr()`
/// carries FR25's warning about when every previous secret stops working.
#[derive(Debug)]
pub(crate) struct RotateOutput {
    pub(crate) pair: RotatedPair,
    /// FR22/FR26: the grace period actually in effect, never absent - omitting
    /// `--grace-period-days` and passing `0` both mean the same "immediate cutover," and are
    /// reported identically. Carried as the command's own already-known request value rather
    /// than re-derived from `pair.previous_secrets_expire_at` (the Platform API's response has
    /// no field this could be read back from in the first place - see `pair_rotate`'s own
    /// `rotated_pair_from`, which computes that field from this same value).
    pub(crate) grace_period_days: i64,
}

impl RotateOutput {
    /// FR25's required text names the pair - fall back to the client ID when the pair has no
    /// name (not explicitly specced; a name reads better than an empty string in prose).
    fn display_name(&self) -> &str {
        self.pair.name.as_deref().unwrap_or(&self.pair.client_id)
    }
}

impl CliOutput for RotateOutput {
    fn text(&self) -> String {
        let mut table = table::get_table();

        table.add_row(vec![
            &Style::WhoAmIKey.paint("Client ID"),
            &self.pair.client_id,
        ]);
        table.add_row(vec![
            &Style::WhoAmIKey.paint("Client Secret"),
            &self.pair.client_secret,
        ]);
        table.add_row(vec![
            &Style::WhoAmIKey.paint("Secret Expires At"),
            &self
                .pair
                .secret_expires_at
                .to_rfc3339_opts(SecondsFormat::Secs, true),
        ]);

        format!("{table}")
    }

    fn stderr(&self) -> Option<String> {
        Some(if self.grace_period_days == 0 {
            format!(
                "Every previous secret for `{}` has stopped working. To keep the old secret \
                working while you roll out a new one, pass `--grace-period-days <DAYS>`.",
                self.display_name()
            )
        } else {
            format!(
                "Every previous secret for `{}` keeps working until {}.",
                self.display_name(),
                self.pair
                    .previous_secrets_expire_at
                    .to_rfc3339_opts(SecondsFormat::Secs, true)
            )
        })
    }

    fn json(&self) -> Result<serde_json::Value, serde_json::Error> {
        Ok(serde_json::json!({
            "key_type": "ClientCredentials",
            "id": self.pair.client_id,
            "client_id": self.pair.client_id,
            "client_secret": self.pair.client_secret,
            "secret_expires_at": self.pair.secret_expires_at.to_rfc3339_opts(SecondsFormat::Secs, true),
            "grace_period_days": self.grace_period_days,
            "previous_secrets_expire_at": self.pair.previous_secrets_expire_at.to_rfc3339_opts(SecondsFormat::Secs, true),
        }))
    }
}

#[cfg(test)]
mod tests {
    use chrono::DateTime;
    use console::strip_ansi_codes;
    use speculoos::prelude::*;

    use super::*;
    use crate::{RoverOutput, options::JsonOutput};

    fn pair() -> RotatedPair {
        RotatedPair {
            client_id: "c_8f2a".to_string(),
            name: Some("ci-deploy".to_string()),
            client_secret: "s_new-secret".to_string(),
            secret_expires_at: DateTime::parse_from_rfc3339("2028-09-25T16:00:00Z").unwrap(),
            previous_secrets_expire_at: DateTime::parse_from_rfc3339("2026-09-25T16:00:00Z")
                .unwrap(),
        }
    }

    // FR26: the rotate JSON payload must include key_type, id, client_id, client_secret,
    // secret_expires_at, grace_period_days, and previous_secrets_expire_at.
    #[test]
    fn json_matches_the_rotate_response_shape() {
        let output = RotateOutput {
            pair: pair(),
            grace_period_days: 1,
        };

        assert_that!(output.json())
            .is_ok()
            .is_equal_to(serde_json::json!({
                "key_type": "ClientCredentials",
                "id": "c_8f2a",
                "client_id": "c_8f2a",
                "client_secret": "s_new-secret",
                "secret_expires_at": "2028-09-25T16:00:00Z",
                "grace_period_days": 1,
                "previous_secrets_expire_at": "2026-09-25T16:00:00Z",
            }));
    }

    // FR26: `grace_period_days` is `0`, never absent, when no grace period was requested.
    #[test]
    fn json_reports_a_zero_grace_period_rather_than_omitting_it() {
        let output = RotateOutput {
            pair: pair(),
            grace_period_days: 0,
        };

        let json = output.json().unwrap();
        assert_that!(json.get("grace_period_days"))
            .is_some()
            .is_equal_to(&serde_json::Value::from(0));
    }

    #[test]
    fn full_envelope_snapshot() {
        let output = RoverOutput::CliOutput(Box::new(RotateOutput {
            pair: pair(),
            grace_period_days: 1,
        }));
        let envelope = JsonOutput::from(&output);

        insta::assert_json_snapshot!(envelope);
    }

    #[test]
    fn text_snapshot() {
        let output = RotateOutput {
            pair: pair(),
            grace_period_days: 1,
        };

        insta::assert_snapshot!(strip_ansi_codes(&output.text()).to_string());
    }

    // FR25: zero grace period - "has stopped working", names the flag to use instead.
    #[test]
    fn stderr_states_an_immediate_cutover_for_a_zero_grace_period() {
        let output = RotateOutput {
            pair: pair(),
            grace_period_days: 0,
        };

        assert_that!(output.stderr()).is_some().is_equal_to(
            "Every previous secret for `ci-deploy` has stopped working. To keep the old secret \
            working while you roll out a new one, pass `--grace-period-days <DAYS>`."
                .to_string(),
        );
    }

    // FR25: non-zero grace period - names when the previous secrets actually expire.
    #[test]
    fn stderr_names_the_expiry_time_for_a_non_zero_grace_period() {
        let output = RotateOutput {
            pair: pair(),
            grace_period_days: 1,
        };

        assert_that!(output.stderr()).is_some().is_equal_to(
            "Every previous secret for `ci-deploy` keeps working until 2026-09-25T16:00:00Z."
                .to_string(),
        );
    }

    // FR25: falls back to the client ID when the pair has no name.
    #[test]
    fn stderr_falls_back_to_the_client_id_when_the_pair_has_no_name() {
        let output = RotateOutput {
            pair: RotatedPair {
                name: None,
                ..pair()
            },
            grace_period_days: 0,
        };

        assert_that!(output.stderr())
            .is_some()
            .is_equal_to("Every previous secret for `c_8f2a` has stopped working. To keep the old secret working while you roll out a new one, pass `--grace-period-days <DAYS>`.".to_string());
    }
}

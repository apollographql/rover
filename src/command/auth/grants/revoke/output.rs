use rover_client::operations::api_key::pair_list::OAuthClientPair;
use serde::Serialize;

use crate::{command::CliOutput, utils::table};

/// FR67's `kind`: which sort of OAuth client a sweep revoked under.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ClientKind {
    /// Rover's own OAuth client - every personal browser and `--no-browser` login.
    Rover,
    /// A client-credential pair registered in the organization.
    ClientCredentials,
}

/// One OAuth client the per-user sweep revokes under (spec FR55).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SweptClient {
    pub(crate) client_id: String,
    pub(crate) name: Option<String>,
    pub(crate) kind: ClientKind,
}

impl SweptClient {
    /// Rover's own OAuth client, under the effective `APOLLO_OAUTH_CLIENT_ID` (FR67).
    pub(crate) fn rover(client_id: impl Into<String>) -> Self {
        Self {
            client_id: client_id.into(),
            name: Some("Rover".to_string()),
            kind: ClientKind::Rover,
        }
    }

    pub(crate) fn pair(pair: &OAuthClientPair) -> Self {
        Self {
            client_id: pair.client_id.clone(),
            name: pair.name.clone(),
            kind: ClientKind::ClientCredentials,
        }
    }

    /// How FR57's prompt names this client. Rover's own line discloses its scope ("in every
    /// organization") but never details another organization's grants (§6).
    pub(crate) fn prompt_label(&self) -> String {
        match self.kind {
            ClientKind::Rover => {
                "Rover (personal browser and device-code logins, in every organization)".to_string()
            }
            ClientKind::ClientCredentials => self.id_label(),
        }
    }

    /// How FR63's partial-failure list names this client: by name and ID (FR62).
    fn id_label(&self) -> String {
        match (&self.kind, &self.name) {
            (ClientKind::Rover, _) => format!("Rover (`{}`)", self.client_id),
            (ClientKind::ClientCredentials, Some(name)) => {
                format!("`{name}` (`{}`)", self.client_id)
            }
            (ClientKind::ClientCredentials, None) => format!("`{}`", self.client_id),
        }
    }
}

/// The outcome of revoking under one client (FR62). `error` is `None` when it was revoked -
/// including when the user held no grant there to revoke.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ClientOutcome {
    pub(crate) client: SweptClient,
    pub(crate) error: Option<String>,
}

/// FR57-FR67 (`specs/rover-431-identity-grant-management`): the result of
/// `rover auth grants revoke --org <ORG> --user <USER> --all`. Carried as the success output
/// when every client was revoked or the prompt was declined, and inside
/// `GrantsRevokeError::PartialFailure` when any client failed, so `data` reports every client
/// either way (FR67).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RevokeSweepOutput {
    pub(crate) organization_id: String,
    pub(crate) user_id: String,
    /// `None` when the Platform API reported no member list, so Rover can't tell.
    pub(crate) user_is_member: Option<bool>,
    pub(crate) clients: Vec<ClientOutcome>,
    pub(crate) cancelled: bool,
}

impl RevokeSweepOutput {
    pub(crate) fn failed(&self) -> impl Iterator<Item = &ClientOutcome> {
        self.clients
            .iter()
            .filter(|outcome| outcome.error.is_some())
    }

    /// FR58 when declined, otherwise FR63's success or partial-failure text.
    pub(crate) fn summary(&self) -> String {
        let user_id = &self.user_id;
        let organization_id = &self.organization_id;
        let total = self.clients.len();
        if self.cancelled {
            return "Revocation cancelled. Nothing was revoked.".to_string();
        }
        let failed: Vec<&ClientOutcome> = self.failed().collect();
        if failed.is_empty() {
            return format!(
                "Revoked every grant `{user_id}` holds under {total} OAuth clients in \
                organization `{organization_id}`.\nAccess tokens they already hold keep working \
                until they expire. This doesn't stop `{user_id}` logging in again; remove them \
                from the organization to do that."
            );
        }
        let failures: String = failed
            .iter()
            .map(|outcome| {
                format!(
                    "\n  - {}: {}",
                    outcome.client.id_label(),
                    outcome.error.as_deref().unwrap_or_default()
                )
            })
            .collect();
        format!(
            "Revoked grants for `{user_id}` under {} of {total} OAuth clients in organization \
            `{organization_id}`. Revocation failed under:{failures}\nRun the same command again \
            to retry. Clients already revoked are unaffected by a retry.",
            total - failed.len()
        )
    }
}

impl CliOutput for RevokeSweepOutput {
    /// FR62: every client's outcome, by name and ID. Nothing on a declined prompt.
    fn text(&self) -> String {
        if self.cancelled {
            return String::new();
        }
        let mut table = table::get_table();
        table.set_header(vec!["Client", "Client ID", "Outcome"]);
        for outcome in &self.clients {
            table.add_row(vec![
                outcome.client.name.clone().unwrap_or_default(),
                outcome.client.client_id.clone(),
                outcome_name(outcome).to_string(),
            ]);
        }
        format!("{table}")
    }

    fn stderr(&self) -> Option<String> {
        Some(self.summary())
    }

    /// FR67's shape. `success` is added by the envelope.
    fn json(&self) -> Result<serde_json::Value, serde_json::Error> {
        let clients: Vec<serde_json::Value> = self
            .clients
            .iter()
            .map(|outcome| {
                serde_json::json!({
                    "client_id": outcome.client.client_id,
                    "name": outcome.client.name,
                    "kind": outcome.client.kind,
                    "outcome": outcome_name(outcome),
                    "error": outcome.error,
                })
            })
            .collect();
        Ok(serde_json::json!({
            "organization_id": self.organization_id,
            "user_id": self.user_id,
            "user_is_member": self.user_is_member,
            "clients": clients,
            "cancelled": self.cancelled,
        }))
    }
}

const fn outcome_name(outcome: &ClientOutcome) -> &'static str {
    match outcome.error {
        Some(_) => "failed",
        None => "revoked",
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use console::strip_ansi_codes;
    use speculoos::prelude::*;

    use super::*;
    use crate::{RoverOutput, options::JsonOutput};

    pub(crate) fn rover() -> SweptClient {
        SweptClient::rover("rover")
    }

    pub(crate) fn ci_deploy() -> SweptClient {
        SweptClient {
            client_id: "c_8f2a".to_string(),
            name: Some("ci-deploy".to_string()),
            kind: ClientKind::ClientCredentials,
        }
    }

    pub(crate) fn nightly_checks() -> SweptClient {
        SweptClient {
            client_id: "c_91be".to_string(),
            name: Some("nightly-checks".to_string()),
            kind: ClientKind::ClientCredentials,
        }
    }

    pub(crate) const fn revoked(client: SweptClient) -> ClientOutcome {
        ClientOutcome {
            client,
            error: None,
        }
    }

    fn output(clients: Vec<ClientOutcome>) -> RevokeSweepOutput {
        RevokeSweepOutput {
            organization_id: "acme".to_string(),
            user_id: "user-123".to_string(),
            user_is_member: Some(true),
            clients,
            cancelled: false,
        }
    }

    fn all_revoked() -> RevokeSweepOutput {
        output(vec![
            revoked(rover()),
            revoked(ci_deploy()),
            revoked(nightly_checks()),
        ])
    }

    fn one_failed() -> RevokeSweepOutput {
        output(vec![
            revoked(rover()),
            revoked(ci_deploy()),
            ClientOutcome {
                client: nightly_checks(),
                error: Some("upstream timed out".to_string()),
            },
        ])
    }

    // FR63: the exact success text.
    #[test]
    fn summary_reports_a_full_sweep() {
        assert_that!(all_revoked().summary()).is_equal_to(
            "Revoked every grant `user-123` holds under 3 OAuth clients in organization `acme`.\n\
            Access tokens they already hold keep working until they expire. This doesn't stop \
            `user-123` logging in again; remove them from the organization to do that."
                .to_string(),
        );
    }

    // FR63: the exact partial-failure text, naming every client that needs a retry.
    #[test]
    fn summary_names_every_client_that_failed() {
        assert_that!(one_failed().summary()).is_equal_to(
            "Revoked grants for `user-123` under 2 of 3 OAuth clients in organization `acme`. \
            Revocation failed under:\n  - `nightly-checks` (`c_91be`): upstream timed out\n\
            Run the same command again to retry. Clients already revoked are unaffected by a \
            retry."
                .to_string(),
        );
    }

    // FR62: Rover's own client is named by ID too when it fails.
    #[test]
    fn summary_names_rovers_own_client_by_id_when_it_fails() {
        let output = output(vec![ClientOutcome {
            client: rover(),
            error: Some("boom".to_string()),
        }]);

        assert_that!(output.summary()).is_equal_to(
            "Revoked grants for `user-123` under 0 of 1 OAuth clients in organization `acme`. \
            Revocation failed under:\n  - Rover (`rover`): boom\nRun the same command again to \
            retry. Clients already revoked are unaffected by a retry."
                .to_string(),
        );
    }

    // FR58.
    #[test]
    fn summary_reports_a_declined_prompt() {
        let output = RevokeSweepOutput {
            cancelled: true,
            ..output(vec![])
        };

        assert_that!(output.summary())
            .is_equal_to("Revocation cancelled. Nothing was revoked.".to_string());
        assert_that!(output.text()).is_equal_to(String::new());
    }

    // FR62: one row per client, by name and ID, with its outcome.
    #[test]
    fn text_reports_every_clients_outcome() {
        insta::assert_snapshot!(strip_ansi_codes(&one_failed().text()));
    }

    // FR67: the full envelope for a clean sweep.
    #[test]
    fn json_envelope_for_a_full_sweep_snapshot() {
        let output = RoverOutput::CliOutput(Box::new(all_revoked()));
        insta::assert_json_snapshot!(JsonOutput::from(&output));
    }

    // FR67: a declined prompt is `success: true`, no clients, `cancelled: true`. JSON mode can't
    // produce this today - without `--confirm` it fails with E062 before prompting, and with it
    // there's no prompt to decline - but FR67 defines the shape, so it's pinned here.
    #[test]
    fn json_envelope_for_a_declined_prompt_snapshot() {
        let output = RoverOutput::CliOutput(Box::new(RevokeSweepOutput {
            cancelled: true,
            ..output(vec![])
        }));
        insta::assert_json_snapshot!(JsonOutput::from(&output));
    }
}

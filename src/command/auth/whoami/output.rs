use rover_std::Style;

use crate::{command::CliOutput, utils::table};

/// How the current grant was established, for `rover auth whoami`'s `grant_type`/`Grant Type`
/// reporting (spec `rover-431` FR33/FR34). Distinct from `houston::OauthGrantType`: this type
/// additionally carries `ClientCredentials` (a client-credentials exchange never persists a
/// profile credential at all, so `houston` has no variant for it) and `Unknown` (a stored OAuth
/// login with no recorded grant type - an absence, not a value).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum GrantTypeReport {
    AuthorizationCode,
    DeviceCode,
    ClientCredentials,
    Unknown,
}

impl From<houston::OauthGrantType> for GrantTypeReport {
    fn from(value: houston::OauthGrantType) -> Self {
        match value {
            houston::OauthGrantType::AuthorizationCode => Self::AuthorizationCode,
            houston::OauthGrantType::DeviceCode => Self::DeviceCode,
        }
    }
}

impl GrantTypeReport {
    /// FR33's JSON wire value.
    const fn wire_value(self) -> &'static str {
        match self {
            Self::AuthorizationCode => "authorization_code",
            Self::DeviceCode => "device_code",
            Self::ClientCredentials => "client_credentials",
            Self::Unknown => "unknown",
        }
    }

    /// FR34's text label.
    const fn label(self) -> &'static str {
        match self {
            Self::AuthorizationCode => "Browser login",
            Self::DeviceCode => "Device code (--no-browser)",
            Self::ClientCredentials => "Client credentials",
            Self::Unknown => "Unknown — log in again to record it",
        }
    }
}

/// Adds the `Grant Type` row FR34 requires, omitted entirely when `grant_type` is `None` - the
/// same row-presence rule both `AuthWhoAmIOutput` and `AuthWhoAmILegacyOutput` follow.
fn add_grant_type_row(table: &mut comfy_table::Table, grant_type: Option<GrantTypeReport>) {
    if let Some(grant_type) = grant_type {
        table.add_row(vec![
            &Style::WhoAmIKey.paint("Grant Type"),
            &grant_type.label().to_string(),
        ]);
    }
}

#[derive(Debug)]
pub(super) struct AuthWhoAmIOutput {
    pub(super) email: String,
    pub(super) name: String,
    pub(super) user_id: String,
    pub(super) origin: String,
    pub(super) access_token: String,
    /// `None` renders no `Grant Type` row and JSON `grant_type: null` - this branch (an OAuth
    /// login verified live against the IdP) always has a grant type once `whoami` looks it up,
    /// so `None` only occurs for a stored login from before grant type was recorded ("unknown").
    pub(super) grant_type: Option<GrantTypeReport>,
}

impl CliOutput for AuthWhoAmIOutput {
    fn text(&self) -> String {
        let mut table = table::get_table();

        table.add_row(vec![&Style::WhoAmIKey.paint("Name"), &self.name]);
        table.add_row(vec![&Style::WhoAmIKey.paint("Email"), &self.email]);
        table.add_row(vec![&Style::WhoAmIKey.paint("User ID"), &self.user_id]);
        table.add_row(vec![&Style::WhoAmIKey.paint("Origin"), &self.origin]);
        add_grant_type_row(&mut table, self.grant_type);
        table.add_row(vec![
            &Style::WhoAmIKey.paint("Access Token"),
            &self.access_token,
        ]);

        table.to_string()
    }

    fn json(&self) -> Result<serde_json::Value, serde_json::Error> {
        Ok(serde_json::json!({
            "email": self.email,
            "name": self.name,
            "user_id": self.user_id,
            "origin": self.origin,
            "grant_type": self.grant_type.map(GrantTypeReport::wire_value),
            "access_token": self.access_token,
        }))
    }
}

/// `rover auth whoami`'s rendering for a legacy credential (an env var, a pasted-in API key, or
/// a client-credentials exchange) - the same fields `rover config whoami`'s
/// `RoverOutput::ConfigWhoAmIOutput` renders, plus `grant_type`. Deliberately a separate type
/// rather than adding `grant_type` to `ConfigWhoAmIOutput` itself: `rover config whoami` must
/// not gain this field, not even as `null` (PRD `rover-431` §2.3/§6.1).
#[derive(Debug)]
pub(super) struct AuthWhoAmILegacyOutput {
    pub(super) api_key: String,
    pub(super) graph_id: Option<String>,
    pub(super) graph_title: Option<String>,
    pub(super) key_type: String,
    pub(super) origin: String,
    pub(super) user_id: Option<String>,
    /// `Some(ClientCredentials)` for a client-credentials exchange, `None` otherwise (an env
    /// var or a pasted-in API key has no grant at all).
    pub(super) grant_type: Option<GrantTypeReport>,
}

impl CliOutput for AuthWhoAmILegacyOutput {
    fn text(&self) -> String {
        let mut table = table::get_table();

        table.add_row(vec![&Style::WhoAmIKey.paint("Key Type"), &self.key_type]);

        if let Some(graph_id) = &self.graph_id {
            table.add_row(vec![&Style::WhoAmIKey.paint("Graph ID"), graph_id]);
        }

        if let Some(graph_title) = &self.graph_title {
            table.add_row(vec![&Style::WhoAmIKey.paint("Graph Title"), graph_title]);
        }

        if let Some(user_id) = &self.user_id {
            table.add_row(vec![&Style::WhoAmIKey.paint("User ID"), user_id]);
        }

        table.add_row(vec![&Style::WhoAmIKey.paint("Origin"), &self.origin]);
        add_grant_type_row(&mut table, self.grant_type);
        table.add_row(vec![&Style::WhoAmIKey.paint("API Key"), &self.api_key]);

        table.to_string()
    }

    fn json(&self) -> Result<serde_json::Value, serde_json::Error> {
        Ok(serde_json::json!({
            "key_type": self.key_type,
            "graph_id": self.graph_id,
            "graph_title": self.graph_title,
            "user_id": self.user_id,
            "origin": self.origin,
            "grant_type": self.grant_type.map(GrantTypeReport::wire_value),
            "api_key": self.api_key,
        }))
    }
}

#[cfg(test)]
mod tests {
    use indoc::indoc;
    use rstest::{fixture, rstest};
    use speculoos::prelude::*;

    use super::*;

    #[rstest]
    #[case::authorization_code(
        houston::OauthGrantType::AuthorizationCode,
        GrantTypeReport::AuthorizationCode
    )]
    #[case::device_code(houston::OauthGrantType::DeviceCode, GrantTypeReport::DeviceCode)]
    fn grant_type_report_from_houston_grant_type(
        #[case] houston_grant_type: houston::OauthGrantType,
        #[case] expected: GrantTypeReport,
    ) {
        assert_that!(GrantTypeReport::from(houston_grant_type)).is_equal_to(expected);
    }

    #[rstest]
    #[case::authorization_code(
        GrantTypeReport::AuthorizationCode,
        "authorization_code",
        "Browser login"
    )]
    #[case::device_code(
        GrantTypeReport::DeviceCode,
        "device_code",
        "Device code (--no-browser)"
    )]
    #[case::client_credentials(
        GrantTypeReport::ClientCredentials,
        "client_credentials",
        "Client credentials"
    )]
    #[case::unknown(
        GrantTypeReport::Unknown,
        "unknown",
        "Unknown — log in again to record it"
    )]
    fn grant_type_report_wire_value_and_label(
        #[case] grant_type: GrantTypeReport,
        #[case] expected_wire_value: &str,
        #[case] expected_label: &str,
    ) {
        assert_that!(grant_type.wire_value()).is_equal_to(expected_wire_value);
        assert_that!(grant_type.label()).is_equal_to(expected_label);
    }

    #[fixture]
    fn output() -> AuthWhoAmIOutput {
        AuthWhoAmIOutput {
            email: "grace@apollographql.com".to_string(),
            name: "Grace Hopper".to_string(),
            user_id: "user-123".to_string(),
            origin: "--profile default (OAuth)".to_string(),
            access_token: "an-access-token".to_string(),
            grant_type: Some(GrantTypeReport::AuthorizationCode),
        }
    }

    #[rstest]
    fn text_includes_every_field(output: AuthWhoAmIOutput) {
        let text = temp_env::with_var("NO_COLOR", Some("1"), || output.text());

        assert_that!(text).is_equal_to(
            indoc! {"
                ┌──────────────┬───────────────────────────┐
                │ Name         ┆ Grace Hopper              │
                ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
                │ Email        ┆ grace@apollographql.com   │
                ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
                │ User ID      ┆ user-123                  │
                ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
                │ Origin       ┆ --profile default (OAuth) │
                ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
                │ Grant Type   ┆ Browser login             │
                ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
                │ Access Token ┆ an-access-token           │
                └──────────────┴───────────────────────────┘"}
            .to_string(),
        );
    }

    #[rstest]
    fn json_matches_expected_shape(output: AuthWhoAmIOutput) {
        assert_that!(output.json())
            .is_ok()
            .is_equal_to(serde_json::json!({
                "email": "grace@apollographql.com",
                "name": "Grace Hopper",
                "user_id": "user-123",
                "origin": "--profile default (OAuth)",
                "grant_type": "authorization_code",
                "access_token": "an-access-token",
            }));
    }

    #[rstest]
    fn json_reports_null_grant_type_when_unrecorded(mut output: AuthWhoAmIOutput) {
        output.grant_type = Some(GrantTypeReport::Unknown);

        assert_that!(output.json())
            .is_ok()
            .is_equal_to(serde_json::json!({
                "email": "grace@apollographql.com",
                "name": "Grace Hopper",
                "user_id": "user-123",
                "origin": "--profile default (OAuth)",
                "grant_type": "unknown",
                "access_token": "an-access-token",
            }));
    }

    #[fixture]
    fn legacy_output() -> AuthWhoAmILegacyOutput {
        AuthWhoAmILegacyOutput {
            api_key: "user***********************LOLO".to_string(),
            graph_id: None,
            graph_title: None,
            key_type: "USER".to_string(),
            origin: "$APOLLO_CLIENT_ID".to_string(),
            user_id: Some("user-123".to_string()),
            grant_type: Some(GrantTypeReport::ClientCredentials),
        }
    }

    #[rstest]
    fn legacy_text_includes_every_field(legacy_output: AuthWhoAmILegacyOutput) {
        let text = temp_env::with_var("NO_COLOR", Some("1"), || legacy_output.text());

        assert_that!(text).is_equal_to(
            indoc! {"
                ┌────────────┬─────────────────────────────────┐
                │ Key Type   ┆ USER                            │
                ├╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
                │ User ID    ┆ user-123                        │
                ├╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
                │ Origin     ┆ $APOLLO_CLIENT_ID               │
                ├╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
                │ Grant Type ┆ Client credentials              │
                ├╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
                │ API Key    ┆ user***********************LOLO │
                └────────────┴─────────────────────────────────┘"}
            .to_string(),
        );
    }

    #[rstest]
    fn legacy_json_matches_expected_shape(legacy_output: AuthWhoAmILegacyOutput) {
        assert_that!(legacy_output.json())
            .is_ok()
            .is_equal_to(serde_json::json!({
                "key_type": "USER",
                "graph_id": null,
                "graph_title": null,
                "user_id": "user-123",
                "origin": "$APOLLO_CLIENT_ID",
                "grant_type": "client_credentials",
                "api_key": "user***********************LOLO",
            }));
    }

    #[rstest]
    fn legacy_json_reports_null_grant_type_for_an_api_key(
        mut legacy_output: AuthWhoAmILegacyOutput,
    ) {
        legacy_output.grant_type = None;

        assert_that!(legacy_output.json())
            .is_ok()
            .is_equal_to(serde_json::json!({
                "key_type": "USER",
                "graph_id": null,
                "graph_title": null,
                "user_id": "user-123",
                "origin": "$APOLLO_CLIENT_ID",
                "grant_type": null,
                "api_key": "user***********************LOLO",
            }));
    }
}

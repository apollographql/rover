use chrono::SecondsFormat;
use rover_client::operations::api_key::pair_create::CreatedPair;
use rover_std::Style;

use crate::{command::CliOutput, utils::table};

/// The `resource_type` a pair's graph-scoped resources carry - see `pair_list`'s own doc comment
/// on the same shape (FR12) for why this operation doesn't filter or interpret the field itself.
const GRAPH_RESOURCE_TYPE: &str = "GRAPH";

/// FR6-FR9 (`specs/rover-431-identity-grant-management`): the result of creating a
/// `client-credentials` pair. The secret is shown exactly once, here - `stderr()` carries FR7's
/// warning about that.
#[derive(Debug)]
pub(crate) struct CreateClientCredentialsOutput {
    pub(crate) pair: CreatedPair,
}

impl CreateClientCredentialsOutput {
    /// `resources` filtered to the ones that are graphs, in the order the Platform API reported
    /// them - matches `pair_list`'s own doc comment on the same shape (FR12).
    fn graphs(&self) -> Vec<&str> {
        self.pair
            .resources
            .iter()
            .filter(|resource| resource.resource_type == GRAPH_RESOURCE_TYPE)
            .map(|resource| resource.resource_id.as_str())
            .collect()
    }
}

impl CliOutput for CreateClientCredentialsOutput {
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
            &Style::WhoAmIKey.paint("Name"),
            self.pair.name.as_deref().unwrap_or(""),
        ]);
        table.add_row(vec![
            &Style::WhoAmIKey.paint("Graphs"),
            &self.graphs().join(", "),
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
        Some("Save this secret now. Rover can't show it again.".to_string())
    }

    fn json(&self) -> Result<serde_json::Value, serde_json::Error> {
        Ok(serde_json::json!({
            "key_type": "ClientCredentials",
            "id": self.pair.client_id,
            "client_id": self.pair.client_id,
            "client_secret": self.pair.client_secret,
            "secret_expires_at": self.pair.secret_expires_at.to_rfc3339_opts(SecondsFormat::Secs, true),
            "name": self.pair.name,
            "graphs": self.graphs(),
            "scopes": self.pair.scopes,
        }))
    }
}

#[cfg(test)]
mod tests {
    use chrono::DateTime;
    use console::strip_ansi_codes;
    use rover_client::operations::api_key::pair_list::PairResource;
    use speculoos::prelude::*;

    use super::*;
    use crate::{RoverOutput, options::JsonOutput};

    fn pair() -> CreatedPair {
        CreatedPair {
            client_id: "c_8f2a".to_string(),
            name: Some("ci-deploy".to_string()),
            client_secret: "s_super-secret".to_string(),
            secret_expires_at: DateTime::parse_from_rfc3339("2028-09-25T16:00:00Z").unwrap(),
            resources: vec![
                PairResource {
                    resource_id: "inventory".to_string(),
                    resource_type: GRAPH_RESOURCE_TYPE.to_string(),
                },
                PairResource {
                    resource_id: "checkout".to_string(),
                    resource_type: GRAPH_RESOURCE_TYPE.to_string(),
                },
            ],
            scopes: vec!["rover:cli".to_string()],
        }
    }

    // The create JSON payload must include key_type, id, client_id, client_secret,
    // secret_expires_at, name, graphs, and scopes.
    #[test]
    fn json_matches_the_create_response_shape() {
        let output = CreateClientCredentialsOutput { pair: pair() };

        assert_that!(output.json())
            .is_ok()
            .is_equal_to(serde_json::json!({
                "key_type": "ClientCredentials",
                "id": "c_8f2a",
                "client_id": "c_8f2a",
                "client_secret": "s_super-secret",
                "secret_expires_at": "2028-09-25T16:00:00Z",
                "name": "ci-deploy",
                "graphs": ["inventory", "checkout"],
                "scopes": ["rover:cli"],
            }));
    }

    // The inner payload above, but through the real `{"json_version", "data", "error"}` envelope
    // every `--format json` response goes through - so a change to the envelope itself, not just
    // this command's own fields, also gets caught.
    #[test]
    fn full_envelope_snapshot() {
        let output =
            RoverOutput::CliOutput(Box::new(CreateClientCredentialsOutput { pair: pair() }));
        let envelope = JsonOutput::from(&output);

        insta::assert_json_snapshot!(envelope);
    }

    // A full-value snapshot rather than field-by-field `.contains()` checks, so a reordered,
    // relabeled, or dropped row fails the test - matches `src/command/check_output.rs`'s pattern.
    #[test]
    fn text_snapshot() {
        let output = CreateClientCredentialsOutput { pair: pair() };
        let text = strip_ansi_codes(&output.text()).to_string();

        insta::assert_snapshot!(text);
    }

    #[test]
    fn stderr_carries_fr7s_exact_warning() {
        let output = CreateClientCredentialsOutput { pair: pair() };

        assert_that!(output.stderr())
            .is_some()
            .is_equal_to("Save this secret now. Rover can't show it again.".to_string());
    }

    #[test]
    fn graphs_excludes_non_graph_resources() {
        let mut pair = pair();
        pair.resources.push(PairResource {
            resource_id: "some-gateway".to_string(),
            resource_type: "GATEWAY".to_string(),
        });
        let output = CreateClientCredentialsOutput { pair };

        assert_that!(output.graphs()).is_equal_to(vec!["inventory", "checkout"]);
    }
}

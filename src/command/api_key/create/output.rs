use chrono::SecondsFormat;
use rover_client::operations::api_key::pair_create::CreatedPair;
use rover_std::Style;

use crate::{command::CliOutput, utils::table};

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
            .filter(|resource| resource.resource_type == "GRAPH")
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
    use rover_client::operations::api_key::pair_list::PairResource;
    use speculoos::prelude::*;

    use super::*;

    fn pair() -> CreatedPair {
        CreatedPair {
            client_id: "c_8f2a".to_string(),
            name: Some("ci-deploy".to_string()),
            client_secret: "s_super-secret".to_string(),
            secret_expires_at: DateTime::parse_from_rfc3339("2028-09-25T16:00:00Z").unwrap(),
            resources: vec![
                PairResource {
                    resource_id: "inventory".to_string(),
                    resource_type: "GRAPH".to_string(),
                },
                PairResource {
                    resource_id: "checkout".to_string(),
                    resource_type: "GRAPH".to_string(),
                },
            ],
            scopes: vec!["rover:cli".to_string()],
        }
    }

    #[test]
    fn json_matches_fr8s_shape() {
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

    #[test]
    fn text_reports_the_client_id_secret_name_graphs_and_expiry() {
        let output = CreateClientCredentialsOutput { pair: pair() };
        let text = output.text();

        assert_that!(text).contains("c_8f2a");
        assert_that!(text).contains("s_super-secret");
        assert_that!(text).contains("ci-deploy");
        assert_that!(text).contains("inventory, checkout");
        assert_that!(text).contains("2028-09-25T16:00:00Z");
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

use chrono::SecondsFormat;
use rover_client::operations::api_key::{list::ApiKey, pair_list::OAuthClientPair};
use rover_std::Style;
use serde_json::{Map, Value};

use crate::{command::CliOutput, utils::table};

/// The `resource_type` a pair's graph-scoped resources carry - see `pair_list`'s own doc comment
/// on the same shape (FR12) for why this operation doesn't filter or interpret the field itself.
/// Duplicated from `create/output.rs`'s own copy rather than shared - promote to a common spot
/// if a third consumer needs it.
const GRAPH_RESOURCE_TYPE: &str = "GRAPH";

/// FR10-FR17 (`specs/rover-431-identity-grant-management`): the result of listing an
/// organization's API keys and client-credential pairs. `None` means `--type` excluded that
/// whole resource from scope, so its JSON field is omitted entirely rather than reported as
/// `[]` (FR11). A genuine pairs-query failure never reaches this type at all - it's reported
/// through `RoverClientError::PairListFailure` instead, so the `data.client_credentials: null`
/// case doesn't need a state here (see `crate::error::mod`'s special-casing of that variant).
#[derive(Debug)]
pub(crate) struct ListOutput {
    pub(crate) keys: Option<Vec<ApiKey>>,
    pub(crate) pairs: Option<Vec<OAuthClientPair>>,
}

/// `resources` filtered to the ones that are graphs, in the order the Platform API reported
/// them - matches `pair_list`'s own doc comment on the same shape (FR12).
fn graphs_of(pair: &OAuthClientPair) -> Vec<&str> {
    pair.resources
        .iter()
        .filter(|resource| resource.resource_type == GRAPH_RESOURCE_TYPE)
        .map(|resource| resource.resource_id.as_str())
        .collect()
}

/// FR15's `client_credentials` entry shape - built by hand rather than derived on
/// `OAuthClientPair` directly, since `key_type`/`id`/`graphs` aren't 1:1 with that type's own
/// fields (matches `create/output.rs`'s identical choice for `CreatedPair`).
fn pair_json(pair: &OAuthClientPair) -> Value {
    serde_json::json!({
        "key_type": "ClientCredentials",
        "id": pair.client_id,
        "client_id": pair.client_id,
        "name": pair.name,
        "graphs": graphs_of(pair),
        "scopes": pair.scopes,
        "created_at": pair.created_at.to_rfc3339_opts(SecondsFormat::Secs, true),
        "created_by": { "id": pair.created_by.id, "type": pair.created_by.kind },
    })
}

/// The API-key table's rendering, shared with `RoverError::print()`'s special-casing of
/// [`rover_client::RoverClientError::PairListFailure`] (`src/error/mod.rs`) - a genuine pairs
/// failure still reports the keys already fetched, in exactly the same shape a success would
/// have.
pub(crate) fn keys_table_text(keys: &[ApiKey]) -> String {
    let mut table = table::get_table();
    table.set_header(vec!["ID", "Name", "Created At", "Expires At"]);
    for key in keys {
        table.add_row(vec![
            key.id.clone(),
            key.name.clone().unwrap_or_default(),
            key.created_at.to_string(),
            key.expires_at
                .map(|timestamp| timestamp.to_string())
                .unwrap_or_else(|| "Never".to_string()),
        ]);
    }
    format!("{table}")
}

/// The same sharing as [`keys_table_text`], for `--format json`'s `data.keys`.
pub(crate) fn keys_json(keys: &[ApiKey]) -> Value {
    serde_json::json!(keys)
}

fn pairs_table_text(pairs: &[OAuthClientPair]) -> String {
    let mut table = table::get_table();
    table.set_header(vec![
        "Name",
        "Client ID",
        "Graphs",
        "Created At",
        "Created By",
    ]);
    for pair in pairs {
        table.add_row(vec![
            pair.name.clone().unwrap_or_default(),
            pair.client_id.clone(),
            graphs_of(pair).join(", "),
            pair.created_at.to_string(),
            format!("{} ({})", pair.created_by.id, pair.created_by.kind),
        ]);
    }
    format!("{table}")
}

impl CliOutput for ListOutput {
    fn text(&self) -> String {
        let mut sections = Vec::new();

        if let Some(keys) = &self.keys {
            sections.push(keys_table_text(keys));
        }

        // FR14: the pairs table appears only when there's at least one to show - an empty or
        // out-of-scope result must leave text output identical to today's, not an empty table.
        if let Some(pairs) = &self.pairs
            && !pairs.is_empty()
        {
            sections.push(format!(
                "{}\n{}",
                Style::Heading.paint("Client-credential pairs"),
                pairs_table_text(pairs)
            ));
        }

        sections.join("\n\n")
    }

    fn json(&self) -> Result<Value, serde_json::Error> {
        let mut data = Map::new();
        if let Some(keys) = &self.keys {
            data.insert("keys".to_string(), keys_json(keys));
        }
        if let Some(pairs) = &self.pairs {
            data.insert(
                "client_credentials".to_string(),
                Value::Array(pairs.iter().map(pair_json).collect()),
            );
        }
        Ok(Value::Object(data))
    }
}

#[cfg(test)]
mod tests {
    use chrono::DateTime;
    use console::strip_ansi_codes;
    use rover_client::operations::api_key::{
        list::ApiKeyBackendType,
        pair_list::{PairActor, PairResource},
    };
    use speculoos::prelude::*;

    use super::*;
    use crate::{RoverOutput, options::JsonOutput};

    fn operator_key() -> ApiKey {
        ApiKey {
            created_at: DateTime::parse_from_rfc3339("2026-01-04T12:00:00Z").unwrap(),
            expires_at: None,
            id: "key-123".to_string(),
            name: Some("router-prod".to_string()),
            key_type: Some(ApiKeyBackendType::Operator),
        }
    }

    fn pair() -> OAuthClientPair {
        OAuthClientPair {
            client_id: "c_8f2a".to_string(),
            name: Some("ci-deploy".to_string()),
            created_at: DateTime::parse_from_rfc3339("2026-09-25T16:00:00Z").unwrap(),
            created_by: PairActor {
                id: "user-123".to_string(),
                kind: "user".to_string(),
            },
            resources: vec![PairResource {
                resource_id: "inventory".to_string(),
                resource_type: "GRAPH".to_string(),
            }],
            scopes: vec!["rover:cli".to_string()],
        }
    }

    #[test]
    fn pair_json_matches_fr15s_client_credentials_shape() {
        assert_that!(pair_json(&pair())).is_equal_to(serde_json::json!({
            "key_type": "ClientCredentials",
            "id": "c_8f2a",
            "client_id": "c_8f2a",
            "name": "ci-deploy",
            "graphs": ["inventory"],
            "scopes": ["rover:cli"],
            "created_at": "2026-09-25T16:00:00Z",
            "created_by": { "id": "user-123", "type": "user" },
        }));
    }

    #[test]
    fn json_omits_keys_and_pairs_when_out_of_scope() {
        let output = ListOutput {
            keys: None,
            pairs: None,
        };

        assert_that!(output.json())
            .is_ok()
            .is_equal_to(serde_json::json!({}));
    }

    #[test]
    fn json_includes_keys_and_pairs_when_in_scope() {
        let output = ListOutput {
            keys: Some(vec![operator_key()]),
            pairs: Some(vec![pair()]),
        };

        let json = output.json().unwrap();
        assert_that!(json.get("keys")).is_some();
        assert_that!(json.get("client_credentials")).is_some();
    }

    #[test]
    fn json_reports_an_empty_but_present_pairs_list_as_an_array_not_omitted() {
        let output = ListOutput {
            keys: Some(vec![]),
            pairs: Some(vec![]),
        };

        assert_that!(output.json())
            .is_ok()
            .is_equal_to(serde_json::json!({ "keys": [], "client_credentials": [] }));
    }

    // FR14: text output is unchanged when there are no pairs to show, whatever the reason.
    #[test]
    fn text_omits_the_pairs_table_when_there_are_none() {
        let output = ListOutput {
            keys: Some(vec![operator_key()]),
            pairs: Some(vec![]),
        };

        assert_that!(output.text()).does_not_contain("Client-credential pairs");
    }

    #[test]
    fn text_includes_the_pairs_table_when_there_are_some() {
        let output = ListOutput {
            keys: Some(vec![operator_key()]),
            pairs: Some(vec![pair()]),
        };

        assert_that!(output.text()).contains("Client-credential pairs");
        assert_that!(output.text()).contains("c_8f2a");
    }

    #[test]
    fn text_snapshot_with_keys_and_pairs() {
        let output = ListOutput {
            keys: Some(vec![operator_key()]),
            pairs: Some(vec![pair()]),
        };

        insta::assert_snapshot!(strip_ansi_codes(&output.text()).to_string());
    }

    #[test]
    fn full_envelope_snapshot() {
        let output = RoverOutput::CliOutput(Box::new(ListOutput {
            keys: Some(vec![operator_key()]),
            pairs: Some(vec![pair()]),
        }));

        insta::assert_json_snapshot!(JsonOutput::from(&output));
    }
}

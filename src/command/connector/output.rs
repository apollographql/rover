//! Output types for the `rover connector` subcommands.
//!
//! These were a single `RoverOutput::ConnectorTestResponse` shared by five
//! subcommands, holding an untyped `String` under a name that only describes
//! one of them. Each subcommand now has its own type, so they can be told
//! apart, named separately, and grow fields independently.
//!
//! Four of them carry whatever their subprocess wrote to stdout, verbatim.
//! That is empty for the subcommands whose subprocess inherits this process's
//! stdout and so writes straight to the terminal — `test`, `generate`, and
//! interactive `analyze`. Their JSON has always reported `"output": ""` for
//! that reason; this keeps doing so rather than redefining the field as part
//! of a refactor.

use serde_json::{Value, json};

use crate::command::{
    CliOutput,
    connector::run::{RunConnector, RunConnectorOutput},
    install::PluginProvenance,
};

/// What `rover connector run` captured from a single connector invocation.
#[derive(Debug)]
pub struct ConnectorRunOutput {
    pub output: RunConnectorOutput,
    /// The plugins this run resolved (FR57).
    pub plugins: Vec<PluginProvenance>,
}

impl CliOutput for ConnectorRunOutput {
    fn text(&self) -> String {
        RunConnector::format_output(&self.output)
    }

    fn json(&self) -> Result<Value, serde_json::Error> {
        Ok(json!({ "output": self.output, "plugins": self.plugins }))
    }
}

/// What `rover connector test` reported: empty, since the run writes straight
/// to the terminal.
#[derive(Debug)]
pub struct ConnectorTestOutput {
    pub output: String,
    /// The plugins this run resolved (FR57).
    pub plugins: Vec<PluginProvenance>,
}

impl CliOutput for ConnectorTestOutput {
    fn text(&self) -> String {
        self.output.clone()
    }

    fn json(&self) -> Result<Value, serde_json::Error> {
        Ok(json!({ "output": self.output, "plugins": self.plugins }))
    }
}

/// The connectors `rover connector list` found in a schema.
#[derive(Debug)]
pub struct ConnectorListOutput {
    pub output: String,
    /// The plugins this run resolved (FR57).
    pub plugins: Vec<PluginProvenance>,
}

impl CliOutput for ConnectorListOutput {
    fn text(&self) -> String {
        self.output.clone()
    }

    fn json(&self) -> Result<Value, serde_json::Error> {
        Ok(json!({ "output": self.output, "plugins": self.plugins }))
    }
}

/// What `rover connector generate` reported: empty, for the same reason as
/// [`ConnectorTestOutput`].
#[cfg(target_os = "macos")]
#[derive(Debug)]
pub struct ConnectorGenerateOutput {
    pub output: String,
    /// The plugins this run resolved (FR57).
    pub plugins: Vec<PluginProvenance>,
}

#[cfg(target_os = "macos")]
impl CliOutput for ConnectorGenerateOutput {
    fn text(&self) -> String {
        self.output.clone()
    }

    fn json(&self) -> Result<Value, serde_json::Error> {
        Ok(json!({ "output": self.output, "plugins": self.plugins }))
    }
}

/// What `rover connector analyze` reported. Empty for the interactive variant,
/// which writes straight to the terminal.
#[cfg(target_os = "macos")]
#[derive(Debug)]
pub struct ConnectorAnalyzeOutput {
    pub output: String,
    /// The plugins this run resolved (FR57).
    pub plugins: Vec<PluginProvenance>,
}

#[cfg(target_os = "macos")]
impl CliOutput for ConnectorAnalyzeOutput {
    fn text(&self) -> String {
        self.output.clone()
    }

    fn json(&self) -> Result<Value, serde_json::Error> {
        Ok(json!({ "output": self.output, "plugins": self.plugins }))
    }
}

#[cfg(test)]
mod tests {
    use camino::Utf8PathBuf;
    use semver::Version;
    use serde_json::json;
    use speculoos::prelude::*;

    use super::*;
    use crate::command::install::{PluginLevel, PluginSource};

    fn supergraph_plugin() -> PluginProvenance {
        PluginProvenance::new(
            "supergraph",
            Version::new(2, 12, 0),
            PluginSource::Installed,
            PluginLevel::Global,
            Utf8PathBuf::from("/home/me/.rover/bin/supergraph-v2.12.0"),
        )
    }

    /// Whatever the subprocess wrote is reproduced exactly, including the
    /// empty string the inherit-stdout subcommands produce.
    #[rstest::rstest]
    #[case::empty("")]
    #[case::content("connector-a\nconnector-b")]
    fn passthrough_reports_its_output_verbatim(#[case] output: &str) {
        let subject = ConnectorListOutput {
            output: output.to_string(),
            plugins: vec![],
        };

        assert_that!(subject.text()).is_equal_to(output.to_string());
        assert_that!(subject.json().unwrap())
            .is_equal_to(json!({ "output": output, "plugins": [] }));
    }

    #[test]
    fn json_reports_the_plugin_the_subcommand_ran_on() {
        let subject = ConnectorListOutput {
            output: String::from("connector-a"),
            plugins: vec![supergraph_plugin()],
        };

        assert_that!(subject.json().unwrap()).is_equal_to(json!({
            "output": "connector-a",
            "plugins": [{
                "name": "supergraph",
                "version": "2.12.0",
                "source": "installed",
                "level": "global",
                "path": "/home/me/.rover/bin/supergraph-v2.12.0",
            }],
        }));
    }

    /// Each subcommand keeps the `{"output": ...}` shape it had while they all
    /// shared one variant, so nothing reading that field has to change.
    #[test]
    fn every_passthrough_keeps_the_shared_json_shape() {
        let shapes = [
            ConnectorTestOutput {
                output: String::from("x"),
                plugins: vec![],
            }
            .json()
            .unwrap(),
            ConnectorListOutput {
                output: String::from("x"),
                plugins: vec![],
            }
            .json()
            .unwrap(),
        ];

        for shape in shapes {
            assert_that!(shape).is_equal_to(json!({ "output": "x", "plugins": [] }));
        }
    }

    #[test]
    fn run_reports_the_parsed_response_rather_than_a_string() {
        let response: RunConnectorOutput =
            serde_json::from_str(r#"{"request":null,"response":null,"error":"boom"}"#).unwrap();

        let actual = ConnectorRunOutput {
            output: response,
            plugins: vec![],
        }
        .json()
        .unwrap();

        assert_that!(actual).is_equal_to(json!({
            "output": { "request": null, "response": null, "error": "boom" },
            "plugins": [],
        }));
    }
}

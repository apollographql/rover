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
};

/// What `rover connector run` captured from a single connector invocation.
#[derive(Debug)]
pub struct ConnectorRunOutput(pub RunConnectorOutput);

impl CliOutput for ConnectorRunOutput {
    fn text(&self) -> String {
        RunConnector::format_output(&self.0)
    }

    fn json(&self) -> Result<Value, serde_json::Error> {
        Ok(json!({ "output": self.0 }))
    }
}

/// What `rover connector test` reported: empty, since the run writes straight
/// to the terminal.
#[derive(Debug)]
pub struct ConnectorTestOutput(pub String);

impl CliOutput for ConnectorTestOutput {
    fn text(&self) -> String {
        self.0.clone()
    }

    fn json(&self) -> Result<Value, serde_json::Error> {
        Ok(json!({ "output": self.0 }))
    }
}

/// The connectors `rover connector list` found in a schema.
#[derive(Debug)]
pub struct ConnectorListOutput(pub String);

impl CliOutput for ConnectorListOutput {
    fn text(&self) -> String {
        self.0.clone()
    }

    fn json(&self) -> Result<Value, serde_json::Error> {
        Ok(json!({ "output": self.0 }))
    }
}

/// What `rover connector generate` reported: empty, for the same reason as
/// [`ConnectorTestOutput`].
#[cfg(target_os = "macos")]
#[derive(Debug)]
pub struct ConnectorGenerateOutput(pub String);

#[cfg(target_os = "macos")]
impl CliOutput for ConnectorGenerateOutput {
    fn text(&self) -> String {
        self.0.clone()
    }

    fn json(&self) -> Result<Value, serde_json::Error> {
        Ok(json!({ "output": self.0 }))
    }
}

/// What `rover connector analyze` reported. Empty for the interactive variant,
/// which writes straight to the terminal.
#[cfg(target_os = "macos")]
#[derive(Debug)]
pub struct ConnectorAnalyzeOutput(pub String);

#[cfg(target_os = "macos")]
impl CliOutput for ConnectorAnalyzeOutput {
    fn text(&self) -> String {
        self.0.clone()
    }

    fn json(&self) -> Result<Value, serde_json::Error> {
        Ok(json!({ "output": self.0 }))
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use speculoos::prelude::*;

    use super::*;

    /// Whatever the subprocess wrote is reproduced exactly, including the
    /// empty string the inherit-stdout subcommands produce.
    #[rstest::rstest]
    #[case::empty("")]
    #[case::content("connector-a\nconnector-b")]
    fn passthrough_reports_its_output_verbatim(#[case] output: &str) {
        let subject = ConnectorListOutput(output.to_string());

        assert_that!(subject.text()).is_equal_to(output.to_string());
        assert_that!(subject.json().unwrap()).is_equal_to(json!({ "output": output }));
    }

    /// Each subcommand keeps the `{"output": ...}` shape it had while they all
    /// shared one variant, so nothing reading this has to change.
    #[test]
    fn every_passthrough_keeps_the_shared_json_shape() {
        let shapes = [
            ConnectorTestOutput(String::from("x")).json().unwrap(),
            ConnectorListOutput(String::from("x")).json().unwrap(),
        ];

        for shape in shapes {
            assert_that!(shape).is_equal_to(json!({ "output": "x" }));
        }
    }

    #[test]
    fn run_reports_the_parsed_response_rather_than_a_string() {
        let response: RunConnectorOutput =
            serde_json::from_str(r#"{"request":null,"response":null,"error":"boom"}"#).unwrap();

        let actual = ConnectorRunOutput(response).json().unwrap();

        assert_that!(actual).is_equal_to(json!({
            "output": { "request": null, "response": null, "error": "boom" },
        }));
    }
}

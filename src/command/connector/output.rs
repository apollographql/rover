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
//!
//! Which is why every type here returns `true` from
//! [`CliOutput::prints_empty_output`]. The trait's default reads an empty
//! `text()` as nothing to print, which is right for a command whose result is
//! just "it worked" — but for these, empty is the payload, and the variants
//! they replaced returned it unconditionally. The difference is visible:
//! `--output <file>` writes an empty file for an empty payload and creates no
//! file at all for none, so taking the default would stop
//! `rover connector test -o out.txt` from leaving an `out.txt` behind.
//!
//! `run` is in that list for a less obvious reason than the other four. Its
//! `text()` is [`RunConnector::format_output`], which appends only under
//! `if let Some(..)` and so returns the empty string when the response carries
//! no request, response or error.

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

    /// Empty is a payload here, not an absence — see the module docs.
    fn prints_empty_output(&self) -> bool {
        true
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

    /// Empty is a payload here, not an absence — see the module docs.
    fn prints_empty_output(&self) -> bool {
        true
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

    /// Empty is a payload here, not an absence — see the module docs.
    fn prints_empty_output(&self) -> bool {
        true
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

    /// Empty is a payload here, not an absence — see the module docs.
    fn prints_empty_output(&self) -> bool {
        true
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

    /// Empty is a payload here, not an absence — see the module docs.
    fn prints_empty_output(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use camino::Utf8PathBuf;
    use semver::Version;
    use serde_json::json;
    use speculoos::prelude::*;

    use super::*;
    use crate::{
        RoverOutput,
        command::install::{PluginLevel, PluginSource},
    };

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
    ///
    /// A case per subcommand rather than one test looping over all of them: a
    /// loop stops at the first mismatch and prints the value without naming the
    /// subcommand that produced it.
    ///
    /// Non-empty `plugins` on purpose: an empty array serialises the same
    /// whatever the element type does, so `vec![]` here would pass for every
    /// one of these hand-written `json!` bodies without proving that any of
    /// them renders a provenance correctly.
    ///
    /// The first case is `test_subcommand`, not `test`: naming a case `test`
    /// makes `rstest` generate nothing at all for the function it is on, taking
    /// the other cases with it and compiling clean while it does.
    #[rstest::rstest]
    #[case::test_subcommand(Box::new(ConnectorTestOutput {
        output: String::from("x"),
        plugins: vec![supergraph_plugin()],
    }))]
    #[case::list(Box::new(ConnectorListOutput {
        output: String::from("x"),
        plugins: vec![supergraph_plugin()],
    }))]
    #[cfg_attr(target_os = "macos", case::generate(Box::new(ConnectorGenerateOutput {
        output: String::from("x"),
        plugins: vec![supergraph_plugin()],
    })))]
    #[cfg_attr(target_os = "macos", case::analyze(Box::new(ConnectorAnalyzeOutput {
        output: String::from("x"),
        plugins: vec![supergraph_plugin()],
    })))]
    fn every_passthrough_keeps_the_shared_json_shape(#[case] subject: Box<dyn CliOutput>) {
        assert_that!(subject.json().unwrap()).is_equal_to(json!({
            "output": "x",
            "plugins": [{
                "name": "supergraph",
                "version": "2.12.0",
                "source": "installed",
                "level": "global",
                "path": "/home/me/.rover/bin/supergraph-v2.12.0",
            }],
        }));
    }

    /// The regression this guards: the shared `RoverOutput::CliOutput` arm
    /// collapses an empty `text()` to `None`, and `write_or_print` skips the
    /// whole write-or-print block on `None`. Without the opt-in, `--output
    /// <file>` would silently stop creating the file for the three
    /// subcommands whose output is always empty.
    #[rstest::rstest]
    #[case::empty("")]
    #[case::content("some output")]
    fn empty_output_still_reaches_the_printer(#[case] output: &str) {
        let subject = RoverOutput::CliOutput(Box::new(ConnectorTestOutput {
            output: output.to_string(),
            plugins: vec![supergraph_plugin()],
        }));

        assert_that!(subject.get_stdout().unwrap()).is_equal_to(Some(output.to_string()));
    }

    /// `run` needs the same opt-in as the four passthroughs, for a reason its
    /// type does not show: `format_output` appends only under `if let
    /// Some(..)`, so a response carrying no request, response or error renders
    /// as the empty string and would otherwise be collapsed away.
    #[test]
    fn run_with_nothing_to_report_still_reaches_the_printer() {
        let response: RunConnectorOutput =
            serde_json::from_str(r#"{"request":null,"response":null,"error":null}"#).unwrap();

        // The premise of the test, not an aside: if this ever stops being
        // empty, the case it guards has gone away rather than been fixed.
        assert_that!(RunConnector::format_output(&response)).is_equal_to(String::new());

        let subject = RoverOutput::CliOutput(Box::new(ConnectorRunOutput {
            output: response,
            plugins: vec![supergraph_plugin()],
        }));

        assert_that!(subject.get_stdout().unwrap()).is_equal_to(Some(String::new()));
    }

    #[test]
    fn run_reports_the_parsed_response_rather_than_a_string() {
        let response: RunConnectorOutput =
            serde_json::from_str(r#"{"request":null,"response":null,"error":"boom"}"#).unwrap();

        // A real plugin, not `vec![]`: this is the fifth hand-written `json!`
        // body and the only one the shared-shape loop can't reach, and an empty
        // array serialises identically whatever the element type does.
        let actual = ConnectorRunOutput {
            output: response,
            plugins: vec![supergraph_plugin()],
        }
        .json()
        .unwrap();

        assert_that!(actual).is_equal_to(json!({
            "output": { "request": null, "response": null, "error": "boom" },
            "plugins": [{
                "name": "supergraph",
                "version": "2.12.0",
                "source": "installed",
                "level": "global",
                "path": "/home/me/.rover/bin/supergraph-v2.12.0",
            }],
        }));
    }
}

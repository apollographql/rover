//! What `rover dev` reports when a session ends.

use serde_json::{Value, json};

use crate::command::{CliOutput, install::PluginProvenance};

/// What a finished `rover dev` session reports.
///
/// A session's value is the router it ran, not anything it prints at the end,
/// so there is nothing to say on stdout. It exists as a type rather than
/// `RoverOutput::EmptySuccess` so that the plugins the session resolved have
/// somewhere to go (FR57), and because `EmptySuccess` is shared with a few
/// dozen unrelated commands that must not grow that field.
#[derive(Debug, Default)]
pub struct DevOutput {
    /// Everything the session resolved, in the order it resolved them:
    /// the supergraph binary, the router, and the MCP server when one ran.
    pub plugins: Vec<PluginProvenance>,
}

impl CliOutput for DevOutput {
    fn text(&self) -> String {
        String::new()
    }

    fn json(&self) -> Result<Value, serde_json::Error> {
        Ok(json!({ "plugins": self.plugins }))
    }
}

#[cfg(test)]
mod tests {
    use camino::Utf8PathBuf;
    use semver::Version;
    use speculoos::prelude::*;

    use super::*;
    use crate::{
        RoverOutput,
        command::install::{PluginLevel, PluginSource},
        options::JsonOutput,
    };

    fn plugin(name: &str, version: Version) -> PluginProvenance {
        PluginProvenance::new(
            name,
            version.clone(),
            PluginSource::Downloaded,
            PluginLevel::Global,
            Utf8PathBuf::from(format!("/home/me/.rover/bin/{name}-v{version}")),
        )
    }

    /// A session's stdout stays empty however many plugins it resolved: the
    /// plugins are a JSON field, not something to print at the end.
    #[test]
    fn prints_nothing_on_stdout() {
        let output = RoverOutput::CliOutput(Box::new(DevOutput {
            plugins: vec![plugin("supergraph", Version::new(2, 9, 3))],
        }));

        assert_that!(output.get_stdout().unwrap()).is_equal_to(None);
    }

    #[test]
    fn json_reports_every_plugin_the_session_resolved() {
        let output = DevOutput {
            plugins: vec![
                plugin("supergraph", Version::new(2, 9, 3)),
                plugin("router", Version::new(2, 0, 1)),
            ],
        };

        assert_that!(output.json().unwrap()).is_equal_to(json!({
            "plugins": [
                {
                    "name": "supergraph",
                    "version": "2.9.3",
                    "source": "downloaded",
                    "level": "global",
                    "path": "/home/me/.rover/bin/supergraph-v2.9.3",
                },
                {
                    "name": "router",
                    "version": "2.0.1",
                    "source": "downloaded",
                    "level": "global",
                    "path": "/home/me/.rover/bin/router-v2.0.1",
                },
            ],
        }));
    }

    /// A session that resolved nothing still reports the field, so a consumer
    /// never has to tell "no plugins" apart from "this Rover is too old".
    #[test]
    fn json_reports_an_empty_list_rather_than_omitting_it() {
        let envelope =
            JsonOutput::from(&RoverOutput::CliOutput(Box::new(DevOutput::default()))).to_string();

        assert_that!(envelope).is_equal_to(String::from(
            r#"{"json_version":"1","data":{"plugins":[],"success":true},"error":null}"#,
        ));
    }
}

use serde_json::{Value, json};

use crate::command::{CliOutput, install::PluginProvenance};

/// What `rover lsp` reports when the language server stops.
///
/// The server talks to its editor over the LSP protocol on stdin and stdout
/// for the whole of its life, so there is nothing for it to print as a final
/// payload — and stdout in particular is the protocol's, not ours. This type
/// exists so the session has an output of its own rather than borrowing
/// `RoverOutput::EmptySuccess` from several dozen unrelated commands.
#[derive(Debug, Default)]
pub struct LspOutput {
    /// The plugins the session resolved, at the version last resolved for
    /// each. A long session re-resolves on every file lookup, so this is the
    /// set it ended on rather than everything it ever saw.
    pub plugins: Vec<PluginProvenance>,
}

impl CliOutput for LspOutput {
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
    };

    /// Nothing on stdout is not cosmetic here: the language server has been
    /// speaking LSP over stdout, so a stray line would be a protocol error.
    #[test]
    fn prints_nothing_on_stdout() {
        let output = RoverOutput::CliOutput(Box::new(LspOutput {
            plugins: vec![PluginProvenance::new(
                "supergraph",
                Version::new(2, 9, 3),
                PluginSource::Downloaded,
                PluginLevel::Global,
                Utf8PathBuf::from("/home/me/.rover/bin/supergraph-v2.9.3"),
            )],
        }));

        assert_that!(output.get_stdout().unwrap()).is_equal_to(None);
    }

    #[test]
    fn json_reports_the_plugins_the_session_ended_on() {
        let output = LspOutput {
            plugins: vec![PluginProvenance::new(
                "supergraph",
                Version::new(2, 9, 3),
                PluginSource::Installed,
                PluginLevel::Global,
                Utf8PathBuf::from("/home/me/.rover/bin/supergraph-v2.9.3"),
            )],
        };

        assert_that!(output.json().unwrap()).is_equal_to(json!({
            "plugins": [{
                "name": "supergraph",
                "version": "2.9.3",
                "source": "installed",
                "level": "global",
                "path": "/home/me/.rover/bin/supergraph-v2.9.3",
            }],
        }));
    }
}

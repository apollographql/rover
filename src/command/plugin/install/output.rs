use serde_json::{Value, json};

use crate::command::{CliOutput, install::PluginProvenance};

/// What `rover plugin install` installed.
#[derive(Debug)]
pub struct PluginInstallOutput {
    /// The plugins this run installed (FR57).
    pub plugins: Vec<PluginProvenance>,
}

impl CliOutput for PluginInstallOutput {
    /// Nothing on stdout, as before this type existed: the installer reports
    /// its download on stderr as it happens.
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
    use crate::command::{
        RoverOutput,
        install::{PluginLevel, PluginSource},
    };

    fn installed() -> PluginInstallOutput {
        PluginInstallOutput {
            plugins: vec![PluginProvenance::new(
                "supergraph",
                Version::new(2, 9, 3),
                PluginSource::Downloaded,
                PluginLevel::Global,
                Utf8PathBuf::from("/home/me/.rover/bin/supergraph-v2.9.3"),
            )],
        }
    }

    #[test]
    fn json_reports_each_installed_plugin() {
        assert_that!(installed().json().unwrap()).is_equal_to(json!({
            "plugins": [{
                "name": "supergraph",
                "version": "2.9.3",
                "source": "downloaded",
                "level": "global",
                "path": "/home/me/.rover/bin/supergraph-v2.9.3",
            }]
        }));
    }

    #[test]
    fn text_prints_nothing() {
        let output = RoverOutput::CliOutput(Box::new(installed()));
        assert_that!(output.get_stdout().unwrap()).is_none();
    }
}

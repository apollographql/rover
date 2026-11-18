use camino::Utf8PathBuf;
use semver::Version;
use serde::Serialize;
use serde_json::{Value, json};

use crate::{
    command::{CliOutput, install::PluginLevel},
    plugin::version::PluginName,
};

/// What `rover plugin uninstall` removed.
#[derive(Debug)]
pub struct PluginUninstallOutput {
    pub removed: Vec<RemovedCopy>,
}

/// One copy an uninstall removed.
#[derive(Debug, Serialize)]
pub struct RemovedCopy {
    pub name: PluginName,
    pub version: Version,
    pub level: PluginLevel,
    pub path: Utf8PathBuf,
}

impl CliOutput for PluginUninstallOutput {
    /// Nothing on stdout, as for `rover plugin install`: each removal is
    /// reported on stderr as it happens.
    fn text(&self) -> String {
        String::new()
    }

    fn json(&self) -> Result<Value, serde_json::Error> {
        Ok(json!({ "removed": self.removed }))
    }
}

#[cfg(test)]
mod tests {
    use speculoos::prelude::*;

    use super::*;

    #[test]
    fn json_reports_each_removed_copy() {
        let output = PluginUninstallOutput {
            removed: vec![RemovedCopy {
                name: PluginName::Supergraph,
                version: Version::new(2, 9, 3),
                level: PluginLevel::Project,
                path: Utf8PathBuf::from("/work/app/.rover/bin/supergraph-v2.9.3"),
            }],
        };

        assert_that!(output.json().unwrap()).is_equal_to(json!({
            "removed": [{
                "name": "supergraph",
                "version": "2.9.3",
                "level": "project",
                "path": "/work/app/.rover/bin/supergraph-v2.9.3",
            }]
        }));
    }
}

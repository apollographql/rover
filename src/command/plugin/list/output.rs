use std::fmt::Write;

use camino::Utf8PathBuf;
use semver::Version;
use serde::Serialize;
use serde_json::{Value, json};

use crate::{
    command::{CliOutput, install::PluginLevel},
    plugin::version::PluginName,
};

/// The note below a listing that marks a copy (FR42).
const MARKER_NOTE: &str = "`*` marks the copy Rover uses for each plugin, going by `rover.yaml` and \
     `plugin-versions.lock`. A version flag, an environment variable, or `supergraph.yaml` can \
     select another.";

/// What `rover plugin list` found installed.
#[derive(Debug)]
pub struct PluginListOutput {
    /// The levels listed, in the order they are searched: the project's
    /// first, when there is one.
    pub levels: Vec<ListedLevel>,
}

/// The copies installed at one level.
#[derive(Debug)]
pub struct ListedLevel {
    pub level: PluginLevel,
    pub bin_dir: Utf8PathBuf,
    /// In the order [`PluginName::ALL`] names the plugins, newest first.
    pub copies: Vec<ListedCopy>,
}

/// One installed copy, and whether it is the one a plugin-using command
/// would use.
#[derive(Debug, Serialize)]
pub struct ListedCopy {
    pub name: PluginName,
    pub version: Version,
    pub path: Utf8PathBuf,
    pub selected: bool,
}

impl CliOutput for PluginListOutput {
    fn text(&self) -> String {
        if self.levels.iter().all(|level| level.copies.is_empty()) {
            return if self
                .levels
                .iter()
                .any(|level| level.level == PluginLevel::Project)
            {
                "No plugins are installed in this project or globally.".to_string()
            } else {
                "No plugins are installed globally.".to_string()
            };
        }

        let mut text = String::new();
        for (index, level) in self.levels.iter().enumerate() {
            if index > 0 {
                text.push('\n');
            }
            let heading = match level.level {
                PluginLevel::Project => "Installed in this project",
                PluginLevel::Global => "Installed globally",
            };
            let _ = writeln!(text, "{heading} ({}):", level.bin_dir);
            if level.copies.is_empty() {
                text.push_str("  none\n");
            }
            for copy in &level.copies {
                let marker = if copy.selected { '*' } else { ' ' };
                let _ = writeln!(text, "{marker} {} v{}", copy.name, copy.version);
            }
        }
        let any_selected = self
            .levels
            .iter()
            .flat_map(|level| &level.copies)
            .any(|copy| copy.selected);
        if any_selected {
            let _ = write!(text, "\n{MARKER_NOTE}");
        }
        text.trim_end().to_string()
    }

    fn json(&self) -> Result<Value, serde_json::Error> {
        let levels: Vec<Value> = self
            .levels
            .iter()
            .map(|level| json!({ "level": level.level, "bin_dir": level.bin_dir }))
            .collect();
        let plugins: Vec<Value> = self
            .levels
            .iter()
            .flat_map(|level| {
                level.copies.iter().map(|copy| {
                    json!({
                        "name": copy.name,
                        "version": copy.version,
                        "level": level.level,
                        "path": copy.path,
                        "selected": copy.selected,
                    })
                })
            })
            .collect();
        Ok(json!({ "levels": levels, "plugins": plugins }))
    }
}

#[cfg(test)]
mod tests {
    use speculoos::prelude::*;

    use super::*;

    fn copy(name: PluginName, version: &str, dir: &str, selected: bool) -> ListedCopy {
        ListedCopy {
            name,
            version: Version::parse(version).unwrap(),
            path: Utf8PathBuf::from(format!("{dir}/{name}-v{version}")),
            selected,
        }
    }

    const PROJECT_BIN: &str = "/work/app/.rover/bin";
    const GLOBAL_BIN: &str = "/home/me/.rover/bin";

    fn both_levels() -> PluginListOutput {
        PluginListOutput {
            levels: vec![
                ListedLevel {
                    level: PluginLevel::Project,
                    bin_dir: PROJECT_BIN.into(),
                    copies: vec![copy(PluginName::Router, "2.1.0", PROJECT_BIN, true)],
                },
                ListedLevel {
                    level: PluginLevel::Global,
                    bin_dir: GLOBAL_BIN.into(),
                    copies: vec![
                        copy(PluginName::Supergraph, "2.9.3", GLOBAL_BIN, true),
                        copy(PluginName::Supergraph, "2.9.2", GLOBAL_BIN, false),
                    ],
                },
            ],
        }
    }

    #[test]
    fn text_lists_each_level_and_marks_the_selected_copies() {
        assert_that!(both_levels().text()).is_equal_to(format!(
            "Installed in this project (/work/app/.rover/bin):\n\
             * router v2.1.0\n\
             \n\
             Installed globally (/home/me/.rover/bin):\n\
             * supergraph v2.9.3\n\
             \x20 supergraph v2.9.2\n\
             \n\
             {MARKER_NOTE}"
        ));
    }

    #[test]
    fn text_says_when_a_level_has_nothing_and_omits_the_note_when_nothing_is_marked() {
        let output = PluginListOutput {
            levels: vec![
                ListedLevel {
                    level: PluginLevel::Project,
                    bin_dir: PROJECT_BIN.into(),
                    copies: vec![],
                },
                ListedLevel {
                    level: PluginLevel::Global,
                    bin_dir: GLOBAL_BIN.into(),
                    copies: vec![copy(PluginName::Supergraph, "3.0.0", GLOBAL_BIN, false)],
                },
            ],
        };

        assert_that!(output.text()).is_equal_to(
            "Installed in this project (/work/app/.rover/bin):\n  none\n\n\
             Installed globally (/home/me/.rover/bin):\n  supergraph v3.0.0"
                .to_string(),
        );
    }

    #[test]
    fn text_with_nothing_installed_in_a_project_names_both_levels() {
        let output = PluginListOutput {
            levels: vec![
                ListedLevel {
                    level: PluginLevel::Project,
                    bin_dir: PROJECT_BIN.into(),
                    copies: vec![],
                },
                ListedLevel {
                    level: PluginLevel::Global,
                    bin_dir: GLOBAL_BIN.into(),
                    copies: vec![],
                },
            ],
        };

        assert_that!(output.text())
            .is_equal_to("No plugins are installed in this project or globally.".to_string());
    }

    #[test]
    fn text_with_nothing_installed_globally_names_the_global_level() {
        let output = PluginListOutput {
            levels: vec![ListedLevel {
                level: PluginLevel::Global,
                bin_dir: GLOBAL_BIN.into(),
                copies: vec![],
            }],
        };

        assert_that!(output.text()).is_equal_to("No plugins are installed globally.".to_string());
    }

    #[test]
    fn json_carries_every_copy_with_its_level_and_marker() {
        assert_that!(both_levels().json().unwrap()).is_equal_to(json!({
            "levels": [
                { "level": "project", "bin_dir": "/work/app/.rover/bin" },
                { "level": "global", "bin_dir": "/home/me/.rover/bin" },
            ],
            "plugins": [
                {
                    "name": "router",
                    "version": "2.1.0",
                    "level": "project",
                    "path": "/work/app/.rover/bin/router-v2.1.0",
                    "selected": true,
                },
                {
                    "name": "supergraph",
                    "version": "2.9.3",
                    "level": "global",
                    "path": "/home/me/.rover/bin/supergraph-v2.9.3",
                    "selected": true,
                },
                {
                    "name": "supergraph",
                    "version": "2.9.2",
                    "level": "global",
                    "path": "/home/me/.rover/bin/supergraph-v2.9.2",
                    "selected": false,
                },
            ]
        }));
    }
}

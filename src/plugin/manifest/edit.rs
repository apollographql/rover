//! Changing the `plugins:` entries of a `rover.yaml` on disk.
//!
//! A manifest is written by hand, so a change to it must leave everything else
//! as its author wrote it: comments, key order, quoting, indentation, and line
//! endings. The edit is made on the file's syntax tree rather than by
//! re-serializing what was parsed, and the result is read back before it is
//! written, so an edit that would change anything but the entry it meant to is
//! refused rather than written.

use std::{collections::BTreeMap, str::FromStr, sync::Arc};

use camino::{Utf8Path, Utf8PathBuf};
use rover_std::Fs;
use yaml_edit::{Mapping, ScalarValue, YamlFile};

use super::{RoverManifest, load::unusable};
use crate::{
    command::plugin::ManifestWrite,
    plugin::{
        error::{ManifestProblem, NotUtf8, PluginFailure},
        version::{PluginName, VersionRequest},
    },
};

const PLUGINS: &str = "plugins";

/// Why a manifest's `plugins:` entry can't be edited in place.
#[derive(Debug, thiserror::Error)]
enum Uneditable {
    #[error("`plugins` is written as a flow mapping (`{{ ... }}`)")]
    FlowMapping,
    #[error("`plugins` is not written as a plain mapping")]
    NotAMapping,
    #[error("the file can't be edited without changing more than the plugin: {0}")]
    WouldChangeMore(String),
}

/// A `rover.yaml` read for editing. Nothing is written until [`Self::write`].
pub struct ManifestEditor {
    path: Utf8PathBuf,
    file: YamlFile,
    /// The declarations as the file stands, edits included.
    declared: BTreeMap<PluginName, VersionRequest>,
    /// Everything the manifest says, before any edit.
    original: RoverManifest,
    /// Whether the file's lines end in `\r\n`, which the edit keeps.
    crlf: bool,
}

impl ManifestEditor {
    /// Read the manifest at `path` for editing. A file that isn't there is
    /// edited as an empty one, and created by [`Self::write`]. A file that
    /// can't be read, or isn't a valid manifest, fails as it does anywhere
    /// else a manifest is read.
    pub fn open(path: &Utf8Path) -> Result<Self, Box<PluginFailure>> {
        let contents = match Fs::read_if_present(path) {
            Ok(Some(bytes)) => String::from_utf8(bytes).map_err(|error| {
                unusable(path, ManifestProblem::Malformed(Arc::new(NotUtf8(error))))
            })?,
            Ok(None) => String::new(),
            Err(source) => {
                return Err(unusable(
                    path,
                    ManifestProblem::Unreadable(Arc::new(source)),
                ));
            }
        };
        let original = RoverManifest::parse(path, &contents)?;
        let crlf = contents.contains("\r\n");
        let file = YamlFile::from_str(&contents.replace("\r\n", "\n"))
            .map_err(|error| uneditable(path, Uneditable::WouldChangeMore(error.to_string())))?;
        let declared = declarations(&original);

        Ok(Self {
            path: path.to_path_buf(),
            file,
            declared,
            original,
            crlf,
        })
    }

    /// Declare `plugin` at `request`, replacing any declaration it has.
    pub fn set(
        &mut self,
        plugin: PluginName,
        request: &VersionRequest,
    ) -> Result<(), Box<PluginFailure>> {
        let document = self.file.ensure_document();
        if document.as_mapping().is_none() {
            // An empty file, or one holding only comments, has no mapping
            // for `plugins` to go in until one is made.
            document.set(PLUGINS, Mapping::new());
        }
        let root = document
            .as_mapping()
            .ok_or_else(|| uneditable(&self.path, Uneditable::NotAMapping))?;
        let plugins = match self.plugins(&root)? {
            Some(plugins) => plugins,
            // Absent or empty: a block mapping replaces it.
            None => {
                root.set(PLUGINS, Mapping::new());
                root.get_mapping(PLUGINS)
                    .ok_or_else(|| uneditable(&self.path, Uneditable::NotAMapping))?
            }
        };
        plugins.set(
            plugin.as_str(),
            ScalarValue::double_quoted(request.to_string()),
        );
        self.declared.insert(plugin, request.clone());
        Ok(())
    }

    /// Remove `plugin`'s declaration, returning whether it had one.
    pub fn remove(&mut self, plugin: PluginName) -> Result<bool, Box<PluginFailure>> {
        if !self.declared.contains_key(&plugin) {
            return Ok(false);
        }
        let root = self
            .file
            .ensure_document()
            .as_mapping()
            .ok_or_else(|| uneditable(&self.path, Uneditable::NotAMapping))?;
        let plugins = self
            .plugins(&root)?
            .ok_or_else(|| uneditable(&self.path, Uneditable::NotAMapping))?;
        if plugins.remove(plugin.as_str()).is_none() {
            return Err(uneditable(&self.path, Uneditable::NotAMapping));
        }
        self.declared.remove(&plugin);
        Ok(true)
    }

    /// The file as it would be written, once checked to say exactly what the
    /// edits meant it to and nothing else changed.
    pub fn render(&self) -> Result<String, Box<PluginFailure>> {
        let rendered = self.file.to_string();
        let rendered = if self.crlf {
            rendered.replace('\n', "\r\n")
        } else {
            rendered
        };

        let reread = RoverManifest::parse(&self.path, &rendered).map_err(|failure| {
            uneditable(&self.path, Uneditable::WouldChangeMore(failure.to_string()))
        })?;
        let unchanged = RoverManifest {
            plugins: reread.plugins.clone(),
            ..self.original.clone()
        };
        if declarations(&reread) != self.declared || unchanged != reread {
            return Err(uneditable(
                &self.path,
                Uneditable::WouldChangeMore("the result reads differently".to_string()),
            ));
        }
        Ok(rendered)
    }

    /// Write the edited manifest, replacing what is there. An interrupted
    /// write leaves the old manifest, not half of the new one. The directory
    /// must already exist.
    pub fn write(&self, _permit: &ManifestWrite) -> Result<(), Box<PluginFailure>> {
        let rendered = self.render()?;
        Fs::write_file_atomically(&self.path, rendered)
            .map_err(|source| unusable(&self.path, ManifestProblem::Unwritable(Arc::new(source))))
    }

    /// The `plugins` mapping in `root`, or `None` when there is none to
    /// edit: no `plugins` key, or one set to nothing.
    fn plugins(&self, root: &Mapping) -> Result<Option<Mapping>, Box<PluginFailure>> {
        let Some(node) = root.get(PLUGINS) else {
            return Ok(None);
        };
        if node.as_scalar().is_some_and(|scalar| scalar.is_null()) {
            return Ok(None);
        }
        let plugins = root
            .get_mapping(PLUGINS)
            .ok_or_else(|| uneditable(&self.path, Uneditable::NotAMapping))?;
        if plugins.is_flow_style() {
            return Err(uneditable(&self.path, Uneditable::FlowMapping));
        }
        Ok(Some(plugins))
    }
}

/// What `manifest` declares, plugin by plugin.
fn declarations(manifest: &RoverManifest) -> BTreeMap<PluginName, VersionRequest> {
    manifest
        .plugins
        .iter()
        .map(|(plugin, declaration)| (plugin, declaration.request.clone()))
        .collect()
}

fn uneditable(path: &Utf8Path, why: Uneditable) -> Box<PluginFailure> {
    unusable(path, ManifestProblem::Uneditable(Arc::new(why)))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use assert_fs::TempDir;
    use indoc::indoc;
    use rstest::rstest;
    use speculoos::prelude::*;

    use super::*;
    use crate::plugin::{error::printed, manifest::MANIFEST_FILE};

    const COMMENTED: &str = indoc! {r#"
        # This project's Rover setup.
        settings:
          # Keep telemetry off here.
          APOLLO_TELEMETRY_DISABLED: true   # for CI too

        # Pinned plugins.
        plugins:
          # The router we test against.
          router: "=2.1.0"
          supergraph: '2'  # composition
    "#};

    /// A manifest holding `contents`, or none at all, and its path.
    fn manifest(contents: Option<&str>) -> (TempDir, Utf8PathBuf) {
        let temp = TempDir::new().unwrap();
        let path = Utf8PathBuf::try_from(temp.path().join(MANIFEST_FILE)).unwrap();
        if let Some(contents) = contents {
            fs::write(&path, contents).unwrap();
        }
        (temp, path)
    }

    fn exact(version: &str) -> VersionRequest {
        format!("={version}").parse().unwrap()
    }

    /// `contents` with `edit` applied, rendered.
    fn edited(
        contents: Option<&str>,
        edit: impl FnOnce(&mut ManifestEditor) -> Result<(), Box<PluginFailure>>,
    ) -> Result<String, String> {
        let (_temp, path) = manifest(contents);
        let mut editor = ManifestEditor::open(&path).map_err(|err| printed(*err))?;
        edit(&mut editor).map_err(|err| printed(*err))?;
        editor.render().map_err(|err| printed(*err))
    }

    #[test]
    fn removing_a_plugin_leaves_the_rest_of_the_file_as_written() {
        let result = edited(Some(COMMENTED), |editor| {
            assert_that!(editor.remove(PluginName::Supergraph))
                .is_ok()
                .is_true();
            Ok(())
        });

        assert_that!(result).is_ok().is_equal_to(
            indoc! {r#"
                # This project's Rover setup.
                settings:
                  # Keep telemetry off here.
                  APOLLO_TELEMETRY_DISABLED: true   # for CI too

                # Pinned plugins.
                plugins:
                  # The router we test against.
                  router: "=2.1.0"
            "#}
            .to_string(),
        );
    }

    #[test]
    fn setting_a_declared_plugin_replaces_only_its_version() {
        let result = edited(Some(COMMENTED), |editor| {
            editor.set(PluginName::Supergraph, &exact("2.9.3"))
        });

        assert_that!(result)
            .is_ok()
            .is_equal_to(COMMENTED.replace("supergraph: '2'", "supergraph: \"=2.9.3\""));
    }

    #[test]
    fn setting_a_new_plugin_adds_it_after_the_others() {
        let result = edited(Some(COMMENTED), |editor| {
            editor.set(PluginName::ApolloMcpServer, &VersionRequest::Latest)
        });

        assert_that!(result)
            .is_ok()
            .is_equal_to(format!("{COMMENTED}  apollo-mcp-server: \"latest\"\n"));
    }

    #[rstest]
    #[case::no_file(None, "plugins:\n  router: \"=2.1.0\"\n")]
    #[case::an_empty_file(Some(""), "plugins:\n  router: \"=2.1.0\"\n")]
    #[case::only_a_comment(
        Some("# Nothing yet.\n"),
        "# Nothing yet.\nplugins:\n  router: \"=2.1.0\"\n"
    )]
    #[case::only_settings(
        Some("settings:\n  APOLLO_TELEMETRY_DISABLED: true\n"),
        "settings:\n  APOLLO_TELEMETRY_DISABLED: true\nplugins:\n  router: \"=2.1.0\"\n"
    )]
    #[case::plugins_set_to_nothing(
        Some("plugins:\nsettings:\n  APOLLO_TELEMETRY_DISABLED: true\n"),
        "plugins:\n  router: \"=2.1.0\"\nsettings:\n  APOLLO_TELEMETRY_DISABLED: true\n"
    )]
    fn setting_a_plugin_where_none_are_declared_makes_a_plugins_mapping(
        #[case] contents: Option<&str>,
        #[case] expected: &str,
    ) {
        let result = edited(contents, |editor| {
            editor.set(PluginName::Router, &exact("2.1.0"))
        });

        assert_that!(result)
            .is_ok()
            .is_equal_to(expected.to_string());
    }

    #[rstest]
    #[case::not_declared(Some("plugins:\n  router: \"2\"\n"))]
    #[case::no_plugins(Some("settings:\n  APOLLO_TELEMETRY_DISABLED: true\n"))]
    #[case::no_file(None)]
    fn removing_a_plugin_that_is_not_declared_changes_nothing(#[case] contents: Option<&str>) {
        let result = edited(contents, |editor| {
            assert_that!(editor.remove(PluginName::Supergraph))
                .is_ok()
                .is_false();
            Ok(())
        });

        assert_that!(result)
            .is_ok()
            .is_equal_to(contents.unwrap_or_default().to_string());
    }

    #[test]
    fn windows_line_endings_are_kept_on_every_line() {
        let contents =
            "settings:\r\n  APOLLO_TELEMETRY_DISABLED: true\r\nplugins:\r\n  router: \"2\"\r\n";

        let result = edited(Some(contents), |editor| {
            editor.set(PluginName::Supergraph, &exact("2.9.3"))
        });

        assert_that!(result)
            .is_ok()
            .is_equal_to(format!("{contents}  supergraph: \"=2.9.3\"\r\n"));
    }

    #[test]
    fn a_flow_mapping_is_refused_rather_than_rewritten() {
        let (_temp, path) = manifest(Some("plugins: {router: \"2\"}\n"));
        let mut editor = ManifestEditor::open(&path).unwrap();

        let set = editor.set(PluginName::Supergraph, &exact("2.9.3"));
        let removed = editor.remove(PluginName::Router);

        let expected = format!(
            "error[E052]: Rover can't update the plugins declared in `{path}`.\n\
             \n\
             Caused by:\n    \
             `plugins` is written as a flow mapping (`{{ ... }}`)\n        \
             Write `plugins:` in `{path}` as a block mapping of plugin names to versions, or make the change by hand, then re-run the command.\n"
        );
        assert_that!(set.map_err(|err| printed(*err))).is_err_containing(expected.clone());
        assert_that!(removed.map_err(|err| printed(*err))).is_err_containing(expected);
    }

    #[test]
    fn an_invalid_manifest_fails_as_it_does_when_read() {
        let (_temp, path) = manifest(Some("plugins: [router]\n"));

        let opened = ManifestEditor::open(&path).map(|_| ());

        assert_that!(opened.map_err(|err| err.to_string()))
            .is_err_containing(format!("`{path}` is not a valid manifest."));
    }

    #[test]
    fn writing_puts_the_edit_on_disk() {
        let (_temp, path) = manifest(Some(COMMENTED));
        let mut editor = ManifestEditor::open(&path).unwrap();
        editor.remove(PluginName::Router).unwrap();

        editor.write(&ManifestWrite::for_tests()).unwrap();

        assert_that!(fs::read_to_string(&path).unwrap())
            .is_equal_to(COMMENTED.replace("  router: \"=2.1.0\"\n", ""));
    }

    #[test]
    fn a_manifest_that_cannot_be_written_says_so() {
        let (temp, _) = manifest(None);
        // A directory where the manifest should be can't be replaced by a file.
        let path = Utf8PathBuf::try_from(temp.path().join("rover.yaml.d")).unwrap();
        let mut editor = ManifestEditor::open(&path.join(MANIFEST_FILE)).unwrap();
        editor.set(PluginName::Router, &exact("2.1.0")).unwrap();

        let written = editor.write(&ManifestWrite::for_tests());

        assert_that!(written.map_err(|err| err.to_string())).is_err_containing(format!(
            "Couldn't write the manifest `{}`.",
            path.join(MANIFEST_FILE)
        ));
    }
}

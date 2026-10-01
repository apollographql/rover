//! Reading a `rover.yaml` from disk, and every way doing so can fail.
//!
//! A manifest that is absent is not an error: neither level is required to
//! have one. A manifest that is present and cannot be used always is, and is
//! never mistaken for an absent one.

use std::{fmt, sync::Arc};

use camino::Utf8Path;
use rover_std::Fs;
use serde::{
    Deserializer,
    de::{IgnoredAny, MapAccess, Visitor},
};

use super::RoverManifest;
use crate::plugin::error::{ManifestProblem, NotUtf8, PluginFailure};

/// A manifest's file name, inside its level's `.rover/` directory.
pub const MANIFEST_FILE: &str = "rover.yaml";

/// A manifest at `path` that cannot be used, for `problem`. Boxed, as every
/// error carrying a [`PluginFailure`] is, to keep the `Result` small.
fn unusable(path: &Utf8Path, problem: ManifestProblem) -> Box<PluginFailure> {
    Box::new(PluginFailure::Manifest {
        path: path.to_path_buf(),
        problem,
    })
}

impl RoverManifest {
    /// Read the manifest at `path`, or `None` if there is no file there.
    pub fn load(path: &Utf8Path) -> Result<Option<Self>, Box<PluginFailure>> {
        match Fs::read_if_present(path) {
            Ok(Some(bytes)) => match String::from_utf8(bytes) {
                Ok(contents) => Self::parse(path, &contents).map(Some),
                // Refusing `install_root` still comes first, as it does for
                // every other problem with the file.
                Err(error) if names_install_root(&String::from_utf8_lossy(error.as_bytes())) => {
                    Err(unusable(path, ManifestProblem::UnsupportedInstallRoot))
                }
                Err(error) => Err(unusable(
                    path,
                    ManifestProblem::Malformed(Arc::new(NotUtf8(error))),
                )),
            },
            Ok(None) => Ok(None),
            Err(source) => Err(unusable(
                path,
                ManifestProblem::Unreadable(Arc::new(source)),
            )),
        }
    }

    /// Parse `contents`, read from `path`, rejecting anything this version
    /// of Rover cannot honor.
    pub fn parse(path: &Utf8Path, contents: &str) -> Result<Self, Box<PluginFailure>> {
        // `install_root` refuses the manifest whatever its value, and before
        // anything else in the file is judged: fixing some other mistake
        // should not lead to being told to delete a key just made valid.
        if names_install_root(contents) {
            return Err(unusable(path, ManifestProblem::UnsupportedInstallRoot));
        }

        // A file that is only a null declares nothing, like an empty one.
        // Every manifest the full parse accepts, the probe above has already
        // looked through, so one naming `install_root` never gets this far.
        serde_yaml::from_str::<Option<Self>>(contents)
            .map(Option::unwrap_or_default)
            .map_err(|problem| unusable(path, ManifestProblem::Malformed(Arc::new(problem))))
    }
}

impl RoverManifest {
    /// The `settings:` section of the manifest at `path`, read without
    /// judging anything else in the file: a broken `plugins:` section, or an
    /// `install_root`, refuses the manifest for plugins but not for settings,
    /// and a broken `settings:` section never refuses it for plugins (FR80 of
    /// the profile-configuration spec). Only a file that can't be read, isn't
    /// UTF-8 text, or isn't a single YAML mapping is refused here, as it is
    /// for plugins.
    ///
    /// `None` when there's no file, or the file has no `settings` key;
    /// `Some(Value::Null)` for a `settings:` with nothing after it, so a
    /// caller can tell "present but empty" from "absent".
    pub fn load_settings(path: &Utf8Path) -> Result<Option<serde_yaml::Value>, Box<PluginFailure>> {
        let contents = match Fs::read_if_present(path) {
            Ok(Some(bytes)) => String::from_utf8(bytes).map_err(|error| {
                unusable(path, ManifestProblem::Malformed(Arc::new(NotUtf8(error))))
            })?,
            Ok(None) => return Ok(None),
            Err(source) => {
                return Err(unusable(
                    path,
                    ManifestProblem::Unreadable(Arc::new(source)),
                ));
            }
        };

        serde_yaml::from_str::<Option<serde_yaml::Mapping>>(&contents)
            .map(|manifest| manifest.and_then(|mut manifest| manifest.remove("settings")))
            .map_err(|problem| unusable(path, ManifestProblem::Malformed(Arc::new(problem))))
    }
}

/// Whether the first document in `contents` has a top-level `install_root`
/// key, with any value, however malformed the rest of it is: the probe looks
/// past duplicate keys, keys that aren't strings, and later documents, all of
/// which the full parse rejects.
fn names_install_root(contents: &str) -> bool {
    struct TopLevelKeys;

    impl<'de> Visitor<'de> for TopLevelKeys {
        type Value = bool;

        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("a mapping")
        }

        fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<bool, A::Error> {
            let mut found = false;
            while let Some(key) = map.next_key::<serde_yaml::Value>()? {
                map.next_value::<IgnoredAny>()?;
                found |= key.as_str() == Some("install_root");
            }
            Ok(found)
        }
    }

    serde_yaml::Deserializer::from_str(contents)
        .next()
        .is_some_and(|document| document.deserialize_map(TopLevelKeys).unwrap_or(false))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use assert_fs::TempDir;
    use camino::Utf8PathBuf;
    use indoc::indoc;
    use rstest::rstest;
    use speculoos::prelude::*;

    use super::*;
    use crate::{
        RoverError, RoverErrorCode,
        plugin::{
            error::printed,
            version::{PluginName, VersionRequest},
        },
    };

    fn temp_dir() -> (TempDir, Utf8PathBuf) {
        let temp = TempDir::new().unwrap();
        let root = Utf8PathBuf::try_from(temp.path().to_path_buf()).unwrap();
        (temp, root)
    }

    #[rstest]
    fn an_absent_manifest_is_not_an_error() {
        let (_temp, root) = temp_dir();

        assert_that!(RoverManifest::load(&root.join(MANIFEST_FILE)))
            .is_ok()
            .is_none();
    }

    #[rstest]
    fn a_manifest_on_disk_is_read() {
        let (_temp, root) = temp_dir();
        let path = root.join(MANIFEST_FILE);
        fs::write(
            &path,
            indoc! {r#"
                plugins:
                  router: latest
            "#},
        )
        .unwrap();

        let manifest = RoverManifest::load(&path).unwrap().unwrap();

        assert_that!(
            manifest
                .plugins
                .iter()
                .map(|(plugin, declaration)| (plugin, declaration.request.clone()))
                .collect::<Vec<_>>()
        )
        .is_equal_to(vec![(PluginName::Router, VersionRequest::Latest)]);
    }

    #[rstest]
    fn an_unreadable_manifest_is_an_error_not_an_absence() {
        let (_temp, root) = temp_dir();
        // A directory where the file should be cannot be read as one, on
        // every platform.
        let path = root.join(MANIFEST_FILE);
        fs::create_dir(&path).unwrap();

        let error = RoverManifest::load(&path).expect_err("should not load");

        assert_that!(error.to_string())
            .is_equal_to(format!("Couldn't read the manifest `{path}`."));
        assert_that!(error.next_step().to_string()).is_equal_to(format!(
            "Make sure `{path}` is a file you can read, then re-run the command."
        ));
        assert_that!(error.code()).is_equal_to(RoverErrorCode::E052);
    }

    #[rstest]
    fn a_manifest_under_a_file_named_dot_rover_is_absent() {
        let (_temp, root) = temp_dir();
        fs::write(root.join(".rover"), "").unwrap();

        assert_that!(RoverManifest::load(
            &root.join(".rover").join(MANIFEST_FILE)
        ))
        .is_ok()
        .is_none();
    }

    // Creating a symlink on Windows needs a privilege a test cannot count on.
    #[cfg(unix)]
    #[rstest]
    fn a_manifest_that_links_nowhere_is_not_absent() {
        let (_temp, root) = temp_dir();
        let path = root.join(MANIFEST_FILE);
        std::os::unix::fs::symlink(root.join("moved.yaml"), &path).unwrap();

        let error = RoverManifest::load(&path).expect_err("should not load");

        assert_that!(error.to_string())
            .is_equal_to(format!("Couldn't read the manifest `{path}`."));
    }

    #[cfg(unix)]
    #[rstest]
    fn a_manifest_in_a_rover_dir_that_links_nowhere_is_not_absent() {
        let (_temp, root) = temp_dir();
        std::os::unix::fs::symlink(root.join("dotfiles/rover"), root.join(".rover")).unwrap();
        let path = root.join(".rover").join(MANIFEST_FILE);

        let error = RoverManifest::load(&path).expect_err("should not load");

        assert_that!(error.to_string())
            .is_equal_to(format!("Couldn't read the manifest `{path}`."));
    }

    #[rstest]
    #[case::null("null\n")]
    #[case::tilde("~\n")]
    #[case::comments_only("# nothing yet\n")]
    #[case::document_marker("---\n")]
    fn a_manifest_that_declares_nothing_is_empty(#[case] contents: &str) {
        assert_that!(RoverManifest::parse(Utf8Path::new("rover.yaml"), contents))
            .is_ok()
            .is_equal_to(RoverManifest::default());
    }

    #[rstest]
    fn a_manifest_that_is_not_utf8_and_names_install_root_is_refused_for_install_root() {
        let (_temp, root) = temp_dir();
        let path = root.join(MANIFEST_FILE);
        fs::write(&path, b"install_root: x\nplugins:\n  router: \xff\n").unwrap();

        let error = RoverManifest::load(&path).expect_err("should not load");

        assert_that!(error.to_string()).is_equal_to(format!(
            "`{path}` sets `install_root`, which this version of Rover doesn't support."
        ));
    }

    #[cfg(unix)]
    #[rstest]
    fn a_manifest_below_a_directory_that_links_nowhere_is_not_absent() {
        let (_temp, root) = temp_dir();
        std::os::unix::fs::symlink(root.join("moved"), root.join("project")).unwrap();
        let path = root.join("project/.rover").join(MANIFEST_FILE);

        let error = RoverManifest::load(&path).expect_err("should not load");

        assert_that!(error.to_string())
            .is_equal_to(format!("Couldn't read the manifest `{path}`."));
    }

    #[rstest]
    fn a_manifest_that_is_not_utf8_is_malformed() {
        let (_temp, root) = temp_dir();
        let path = root.join(MANIFEST_FILE);
        fs::write(&path, b"plugins:\n  router: \xff\n").unwrap();

        let error = RoverManifest::load(&path).expect_err("should not load");

        assert_that!(printed(error)).is_equal_to(format!(
            "error[E052]: `{path}` is not a valid manifest.

Caused by:
    0: not UTF-8 \
             text
    1: invalid utf-8 sequence of 1 bytes from index 19
        Fix `{path}`, or \
             remove it, then re-run the command.
"
        ));
    }

    #[rstest]
    #[case::not_yaml(
        indoc! {r#"
            plugins:
              supergraph: "2
        "#},
        "found unexpected end of stream at line 3 column 1, while scanning a quoted scalar at line \
         2 column 15"
    )]
    #[case::plugins_is_not_a_mapping(
        indoc! {r#"
            plugins:
              - supergraph
        "#},
        "plugins: invalid type: sequence, expected a mapping of plugin name to version at line 2 \
         column 3"
    )]
    #[case::an_unknown_plugin(
        indoc! {r#"
            plugins:
              apollo-router: latest
        "#},
        "plugins: `apollo-router` is not a Rover plugin. Valid plugins are `supergraph`, `router`, \
         and `apollo-mcp-server` at line 2 column 3"
    )]
    fn a_malformed_manifest_names_its_path_and_the_problem(
        #[case] contents: &str,
        #[case] problem: &str,
    ) {
        let error = RoverManifest::parse(Utf8Path::new("/work/app/.rover/rover.yaml"), contents)
            .expect_err("should not parse");

        assert_that!(printed(error)).is_equal_to(format!(
            "error[E052]: `/work/app/.rover/rover.yaml` is not a valid manifest.\n\nCaused by:\n    \
             {problem}\n        Fix `/work/app/.rover/rover.yaml`, or remove it, then re-run the \
             command.\n"
        ));
    }

    #[rstest]
    #[case::with_a_value(indoc! {r#"
        install_root: ../vendor/rover
        plugins:
          router: latest
    "#})]
    #[case::with_no_value("install_root:\n")]
    #[case::with_a_value_of_the_wrong_type("install_root: [a]\n")]
    #[case::alongside_another_mistake(indoc! {r#"
        install_root: x
        plugins:
          apollo-router: latest
    "#})]
    #[case::twice(indoc! {r#"
        install_root: a
        install_root: b
    "#})]
    #[case::alongside_a_key_that_is_not_a_string(indoc! {r#"
        ? [a]
        : 1
        install_root: x
    "#})]
    #[case::in_the_first_of_two_documents(indoc! {r#"
        install_root: a
        ---
        plugins: {}
    "#})]
    fn a_manifest_naming_install_root_is_refused(#[case] contents: &str) {
        let error = RoverManifest::parse(Utf8Path::new("/work/app/.rover/rover.yaml"), contents)
            .expect_err("should not parse");

        assert_that!(printed(error)).is_equal_to(
            "error[E052]: `/work/app/.rover/rover.yaml` sets `install_root`, which this version of \
             Rover doesn't support.\n        Remove `install_root` from \
             `/work/app/.rover/rover.yaml`. Plugins install into the `bin` directory next to it.\n"
                .to_string(),
        );
    }

    fn settings_of(contents: &str) -> Result<Option<serde_yaml::Value>, Box<PluginFailure>> {
        let (_temp, root) = temp_dir();
        let path = root.join(MANIFEST_FILE);
        fs::write(&path, contents).unwrap();
        RoverManifest::load_settings(&path)
    }

    #[rstest]
    fn an_absent_manifest_has_no_settings() {
        let (_temp, root) = temp_dir();

        assert_that!(RoverManifest::load_settings(&root.join(MANIFEST_FILE)))
            .is_ok()
            .is_none();
    }

    #[rstest]
    #[case::empty_file("")]
    #[case::null("null\n")]
    #[case::only_plugins("plugins:\n  router: latest\n")]
    fn a_manifest_without_a_settings_key_has_no_settings(#[case] contents: &str) {
        assert_that!(settings_of(contents)).is_ok().is_none();
    }

    #[rstest]
    fn an_empty_settings_key_is_present_but_null() {
        assert_that!(settings_of("settings:\n"))
            .is_ok()
            .is_some()
            .is_equal_to(serde_yaml::Value::Null);
    }

    #[rstest]
    fn the_settings_section_is_handed_back_as_written() {
        let section = settings_of(indoc! {r#"
            settings:
              APOLLO_REGISTRY_URL: https://registry.example.com
              apollo_checks_timeout_seconds: 600
        "#})
        .unwrap()
        .unwrap();

        assert_that!(section).is_equal_to(
            serde_yaml::from_str::<serde_yaml::Value>(indoc! {r#"
                APOLLO_REGISTRY_URL: https://registry.example.com
                apollo_checks_timeout_seconds: 600
            "#})
            .unwrap(),
        );
    }

    /// FR80: the plugin section's rules apply to it and not to `settings:`.
    #[rstest]
    #[case::install_root("install_root: ../vendor\nsettings:\n  APOLLO_KEY: x\n")]
    #[case::an_unknown_plugin("plugins:\n  apollo-router: latest\nsettings:\n  APOLLO_KEY: x\n")]
    #[case::merge_keys("<<: {plugins: {}}\nsettings:\n  APOLLO_KEY: x\n")]
    fn a_manifest_refused_for_plugins_still_has_settings(#[case] contents: &str) {
        assert_that!(settings_of(contents))
            .is_ok()
            .is_some()
            .is_equal_to(serde_yaml::from_str::<serde_yaml::Value>("APOLLO_KEY: x").unwrap());
    }

    #[rstest]
    #[case::not_yaml("settings:\n  APOLLO_REGISTRY_URL: \"https\n")]
    #[case::not_a_mapping("- settings\n")]
    #[case::two_documents("settings: {}\n---\nsettings: {}\n")]
    fn a_manifest_that_isnt_one_yaml_mapping_is_refused_for_settings_too(#[case] contents: &str) {
        let error = settings_of(contents).expect_err("should not load");

        assert_that!(error.code()).is_equal_to(RoverErrorCode::E052);
    }

    #[rstest]
    fn a_manifest_that_is_not_utf8_is_refused_for_settings_too() {
        let (_temp, root) = temp_dir();
        let path = root.join(MANIFEST_FILE);
        fs::write(&path, b"settings:\n  APOLLO_REGISTRY_URL: \xff\n").unwrap();

        let error = RoverManifest::load_settings(&path).expect_err("should not load");

        assert_that!(error.to_string()).is_equal_to(format!("`{path}` is not a valid manifest."));
    }

    /// Boxed, as the loader returns it, a failure a caller wraps still decides
    /// the code and the next step.
    #[rstest]
    fn a_wrapped_manifest_error_keeps_its_code_and_next_step() {
        let failure = RoverManifest::parse(
            Utf8Path::new("/work/app/.rover/rover.yaml"),
            "install_root: ../vendor\n",
        )
        .expect_err("should not parse");
        let next_step = failure.next_step().to_string();

        let error = RoverError::new(anyhow::Error::new(failure).context("reading declarations"));

        assert_that!(error.code()).is_equal_to(Some(RoverErrorCode::E052));
        assert_that!(
            error
                .suggestions()
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
        )
        .is_equal_to(vec![next_step]);
    }
}

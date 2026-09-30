//! Reading a `plugin-versions.lock` from disk, and every way doing so can
//! fail.
//!
//! As with a manifest, a lockfile that is absent is not an error, and one that
//! is present and cannot be used always is. A lockfile in a newer format than
//! this Rover reads is refused before anything else in it is judged: a newer
//! Rover may have changed any part of the file, and saying so is more use
//! than pointing at whichever part changed.

use std::sync::Arc;

use camino::Utf8Path;
use rover_std::Fs;
use serde::Deserialize;

use super::{FORMAT_VERSION, PluginLockfile};
use crate::plugin::error::{LockfileProblem, NotUtf8, PluginFailure};

/// A lockfile at `path` that cannot be used, for `problem`.
fn unusable(path: &Utf8Path, problem: LockfileProblem) -> Box<PluginFailure> {
    Box::new(PluginFailure::Lockfile {
        path: path.to_path_buf(),
        problem,
    })
}

impl PluginLockfile {
    /// Read the lockfile at `path`, or `None` if there is no file there.
    pub fn load(path: &Utf8Path) -> Result<Option<Self>, Box<PluginFailure>> {
        match Fs::read_if_present(path) {
            Ok(Some(bytes)) => match String::from_utf8(bytes) {
                Ok(contents) => Self::parse(path, &contents).map(Some),
                Err(error) => Err(unusable(
                    path,
                    LockfileProblem::Malformed(Arc::new(NotUtf8(error))),
                )),
            },
            Ok(None) => Ok(None),
            Err(source) => Err(unusable(
                path,
                LockfileProblem::Unreadable(Arc::new(source)),
            )),
        }
    }

    /// Parse `contents`, read from `path`.
    pub fn parse(path: &Utf8Path, contents: &str) -> Result<Self, Box<PluginFailure>> {
        if let Some(format_version) = newer_format(contents) {
            return Err(unusable(
                path,
                LockfileProblem::WrittenByNewerRover { format_version },
            ));
        }

        toml::from_str(contents).map_err(|problem| {
            unusable(
                path,
                LockfileProblem::Malformed(Arc::new(NotALockfile(problem))),
            )
        })
    }
}

/// A lockfile that isn't TOML, or isn't a lockfile's shape. `toml` ends its
/// message with a newline, which would leave a blank line in the printed
/// error, so it is trimmed here.
#[derive(Debug, thiserror::Error)]
#[error("{}", .0.to_string().trim_end())]
struct NotALockfile(toml::de::Error);

/// The format version `contents` declares, if it is one newer than this
/// Rover reads, whatever the rest of the file holds.
fn newer_format(contents: &str) -> Option<u64> {
    #[derive(Deserialize)]
    struct Header {
        version: u64,
    }

    toml::from_str::<Header>(contents)
        .ok()
        .map(|header| header.version)
        .filter(|version| *version > FORMAT_VERSION)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use assert_fs::TempDir;
    use camino::Utf8PathBuf;
    use indoc::indoc;
    use rstest::rstest;
    use semver::Version;
    use speculoos::prelude::*;

    use super::*;
    use crate::plugin::{
        error::printed,
        lockfile::{LOCKFILE, LockedPlugin},
        version::{PluginName, VersionRequest},
    };

    const PATH: &str = "/work/app/.rover/plugin-versions.lock";

    fn temp_dir() -> (TempDir, Utf8PathBuf) {
        let temp = TempDir::new().unwrap();
        let root = Utf8PathBuf::try_from(temp.path().to_path_buf()).unwrap();
        (temp, root)
    }

    #[rstest]
    fn an_absent_lockfile_is_not_an_error() {
        let (_temp, root) = temp_dir();

        assert_that!(PluginLockfile::load(&root.join(LOCKFILE)))
            .is_ok()
            .is_none();
    }

    #[rstest]
    fn a_lockfile_on_disk_is_read() {
        let (_temp, root) = temp_dir();
        let path = root.join(LOCKFILE);
        fs::write(
            &path,
            indoc! {r#"
                version = 1

                [[plugins]]
                name = "router"
                requested = "latest"
                resolved = "2.1.0"
            "#},
        )
        .unwrap();

        assert_that!(PluginLockfile::load(&path))
            .is_ok()
            .is_some()
            .is_equal_to(PluginLockfile::from_iter([LockedPlugin {
                name: PluginName::Router,
                requested: VersionRequest::Latest,
                resolved: Version::new(2, 1, 0),
                checksum: None,
            }]));
    }

    #[rstest]
    fn an_unreadable_lockfile_is_an_error_not_an_absence() {
        let (_temp, root) = temp_dir();
        // A directory where the file should be cannot be read as one, on
        // every platform.
        let path = root.join(LOCKFILE);
        fs::create_dir(&path).unwrap();

        let error = PluginLockfile::load(&path).expect_err("should not load");

        assert_that!(error.to_string())
            .is_equal_to(format!("Couldn't read the plugin lockfile `{path}`."));
        assert_that!(error.next_step().to_string()).is_equal_to(format!(
            "Make sure `{path}` is a file you can read, then re-run the command."
        ));
    }

    #[rstest]
    fn a_lockfile_that_is_not_utf8_is_malformed() {
        let (_temp, root) = temp_dir();
        let path = root.join(LOCKFILE);
        fs::write(&path, b"version = 1\nplugins = [\xff]\n").unwrap();

        let error = PluginLockfile::load(&path).expect_err("should not load");

        assert_that!(printed(error)).is_equal_to(format!(
            "error[E052]: `{path}` is not a valid plugin lockfile.

Caused by:
    0: not UTF-8 text
    1: invalid utf-8 sequence of 1 bytes from index 23
        Fix `{path}`, or remove it, then re-run the command.
"
        ));
    }

    #[rstest]
    #[case::a_newer_version_alone("version = 2\n", 2)]
    #[case::a_newer_version_with_entries_this_rover_cannot_read(
        indoc! {r#"
            version = 7
            signature = "abc"

            [plugins.supergraph]
            track = "2.9"
        "#},
        7
    )]
    fn a_lockfile_from_a_newer_rover_is_refused_saying_so(
        #[case] contents: &str,
        #[case] version: u64,
    ) {
        let error =
            PluginLockfile::parse(Utf8Path::new(PATH), contents).expect_err("should not parse");

        assert_that!(printed(error)).is_equal_to(format!(
            "error[E052]: `{PATH}` was written by a newer version of Rover, in lockfile format \
             version {version}.\n        Upgrade Rover to a version that reads `{PATH}`, then \
             re-run the command. This version won't read the file or overwrite it.\n"
        ));
    }

    /// `problem` is the cause as `toml` prints it: a location and an excerpt
    /// of the file, then what is wrong there. A problem with the file as a
    /// whole has no location.
    #[rstest]
    #[case::an_empty_file(
        "",
        indoc! {"
            TOML parse error at line 1, column 1
              |
            1 | 
              | ^
            missing field `version`"}
    )]
    #[case::no_version(
        "plugins = []\n",
        indoc! {"
            TOML parse error at line 1, column 1
              |
            1 | plugins = []
              | ^
            missing field `version`"}
    )]
    #[case::a_version_that_is_not_a_number(
        "version = \"2\"\nplugins = []\n",
        indoc! {r#"
            TOML parse error at line 1, column 11
              |
            1 | version = "2"
              |           ^^^
            invalid type: string "2", expected u64"#}
    )]
    #[case::a_version_that_never_existed(
        "version = 0\nplugins = []\n",
        "unsupported lockfile version `0`"
    )]
    #[case::a_bad_entry(
        indoc! {r#"
            version = 1
            plugins = [{ name = "router", requested = "=2.1.0", resolved = "2.2.0" }]
        "#},
        "`router` is locked at `2.2.0`, which the request `=2.1.0` could not have resolved to"
    )]
    fn a_malformed_lockfile_names_its_path_and_the_problem(
        #[case] contents: &str,
        #[case] problem: &str,
    ) {
        let error =
            PluginLockfile::parse(Utf8Path::new(PATH), contents).expect_err("should not parse");
        let problem = problem
            .lines()
            .map(|line| format!("    {line}"))
            .collect::<Vec<_>>()
            .join("\n");

        assert_that!(printed(error)).is_equal_to(format!(
            "error[E052]: `{PATH}` is not a valid plugin lockfile.\n\nCaused by:\n{problem}\n        \
             Fix `{PATH}`, or remove it, then re-run the command.\n"
        ));
    }
}

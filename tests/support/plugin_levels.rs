//! Fixtures for the two levels plugins can live at.
//!
//! The plugin contract is mostly about which of two levels a plugin comes
//! from, so almost nothing in it can be tested without control over both. This
//! module builds them out of temporary directories.
//!
//! Levels are exported to Rover as environment variables on a spawned command
//! rather than set on this process. Tests in one binary share a process, so a
//! fixture that called `set_var` would leak into whichever test happened to run
//! alongside it.

use std::process::Command;

use camino::{Utf8Path, Utf8PathBuf};
use rstest::fixture;
use tempfile::TempDir;

/// The environment variable that relocates the Rover home directory, and with
/// it the global level.
const HOME_ENV_VAR: &str = "APOLLO_HOME";

/// Where Rover keeps configuration and credentials. Separate from the home
/// directory above, and isolated here for two reasons: a test that reads it
/// would otherwise depend on the developer's own machine state, and a command
/// that writes it — accepting the ELv2 licence does exactly that — would
/// otherwise write into their real configuration.
const CONFIG_HOME_ENV_VAR: &str = "APOLLO_CONFIG_HOME";

/// Rover refuses to fetch a plugin until the ELv2 licence has been accepted,
/// and remembers the answer in the configuration home above. With that home
/// isolated, every run starts unaccepted, so the gate would fire before any
/// plugin is looked for. Accept it up front; a test about the gate itself can
/// set this variable again on its own command, which wins.
const LICENSE_ENV_VAR: &str = "APOLLO_ELV2_LICENSE";

/// A disposable global level — the `.rover/` directory every project on a
/// machine shares — rooted in a temporary directory that is removed when this
/// value is dropped.
///
/// The `.rover/` directory itself is created up front, since a test that wants
/// a global level nearly always wants one that exists. Its `bin/` is not:
/// Rover creates that when it installs something, and a test asserting on a
/// missing plugin needs it genuinely missing.
pub struct GlobalLevel {
    // Held for its `Drop`, which removes the directory tree.
    _root: TempDir,
    home: Utf8PathBuf,
}

impl GlobalLevel {
    pub fn new() -> Self {
        let root = TempDir::new().expect("could not create a temporary directory");

        // The temporary directory is reached through a symlink on macOS
        // (`/var` to `/private/var`), and Rover reports the resolved path. A
        // test comparing the two would fail on that difference alone.
        let home = dunce::canonicalize(root.path()).expect("could not resolve the temporary path");
        let home = Utf8PathBuf::try_from(home).expect("temporary path is not valid UTF-8");

        let level = Self { _root: root, home };
        std::fs::create_dir_all(level.rover_dir()).expect("could not create the global `.rover`");
        std::fs::create_dir_all(level.config_dir())
            .expect("could not create the configuration home");
        level
    }

    /// The Rover home directory: what `APOLLO_HOME` is set to, and the parent
    /// of the global `.rover/`.
    pub fn home(&self) -> &Utf8Path {
        &self.home
    }

    /// The global level itself.
    pub fn rover_dir(&self) -> Utf8PathBuf {
        self.home.join(".rover")
    }

    /// Where a globally installed plugin binary lands.
    pub fn bin_dir(&self) -> Utf8PathBuf {
        self.rover_dir().join("bin")
    }

    /// Rover's configuration and credentials, isolated from the machine's.
    pub fn config_dir(&self) -> Utf8PathBuf {
        self.home.join("config")
    }

    /// The environment a Rover process needs in order to use this level, and
    /// nothing of the machine it runs on.
    pub fn env(&self) -> Vec<(&'static str, String)> {
        vec![
            (HOME_ENV_VAR, self.home.to_string()),
            (CONFIG_HOME_ENV_VAR, self.config_dir().to_string()),
            (LICENSE_ENV_VAR, "accept".to_string()),
        ]
    }

    /// Point a Rover command at this level.
    pub fn apply(&self, command: &mut Command) -> &Self {
        for (key, value) in self.env() {
            command.env(key, value);
        }
        self
    }
}

impl Default for GlobalLevel {
    fn default() -> Self {
        Self::new()
    }
}

#[fixture]
pub fn global_level() -> GlobalLevel {
    GlobalLevel::new()
}

#[cfg(test)]
mod tests {
    use binstall::Installer;
    use rstest::rstest;
    use speculoos::prelude::*;

    use super::*;

    #[rstest]
    fn the_global_level_exists_under_the_home_it_exports(global_level: GlobalLevel) {
        assert_that!(global_level.rover_dir().is_dir()).is_true();
        assert_that!(global_level.rover_dir().starts_with(global_level.home())).is_true();
        assert_that!(global_level.env()).is_equal_to(vec![
            ("APOLLO_HOME", global_level.home().to_string()),
            ("APOLLO_CONFIG_HOME", global_level.config_dir().to_string()),
            ("APOLLO_ELV2_LICENSE", "accept".to_string()),
        ]);
    }

    #[rstest]
    fn a_plugin_directory_is_not_created_until_something_installs(global_level: GlobalLevel) {
        assert_that!(global_level.bin_dir().exists()).is_false();
    }

    #[rstest]
    fn the_configuration_home_is_the_fixtures_own_and_starts_empty(global_level: GlobalLevel) {
        // Rover records the accepted ELv2 licence here. Pointed at the real
        // configuration home, a plugin test would edit the developer's machine.
        assert_that!(global_level.config_dir().is_dir()).is_true();
        assert_that!(global_level.config_dir().join("elv2_license.toml").exists()).is_false();
        assert_that!(global_level.config_dir().starts_with(global_level.home())).is_true();
    }

    #[rstest]
    fn the_bin_directory_is_the_one_rover_itself_would_install_into(global_level: GlobalLevel) {
        // Pins the fixture to Rover's own idea of an install root rather than
        // to a second copy of the same path arithmetic: if `Installer` changes
        // where it puts binaries, this fails instead of the fixture quietly
        // pointing somewhere Rover never looks.
        let installer = Installer {
            binary_name: "rover".to_string(),
            force_install: false,
            executable_location: Utf8PathBuf::from("rover"),
            override_install_path: Some(global_level.home().to_path_buf()),
        };

        let bin_dir = installer
            .get_bin_dir_path()
            .expect("could not compute the install root");

        assert_that!(bin_dir).is_equal_to(global_level.bin_dir());
    }

    #[rstest]
    fn two_levels_never_share_a_directory() {
        let one = GlobalLevel::new();
        let other = GlobalLevel::new();

        assert_that!(one.home()).is_not_equal_to(other.home());
    }

    #[rstest]
    fn the_directory_is_removed_when_the_level_is_dropped() {
        let path = {
            let level = GlobalLevel::new();
            level.rover_dir()
        };

        assert_that!(path.exists()).is_false();
    }
}

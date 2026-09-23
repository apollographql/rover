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

/// A disposable project level: a directory tree with a `.rover/` at its root,
/// removed when this value is dropped.
///
/// Rover finds a project by walking up from the working directory, so the root
/// and the directory a command runs in are deliberately separate — see
/// [`TwoLevels::run_from`].
pub struct ProjectLevel {
    // Held for its `Drop`, which removes the directory tree.
    _root: TempDir,
    root: Utf8PathBuf,
}

impl ProjectLevel {
    /// A project whose `.rover/` exists.
    pub fn new() -> Self {
        let project = Self::without_rover_dir();
        std::fs::create_dir_all(project.rover_dir())
            .expect("could not create the project `.rover`");
        project
    }

    /// A directory tree with no `.rover/` anywhere in it, for the case where
    /// walking up finds no project at all.
    pub fn without_rover_dir() -> Self {
        let root = TempDir::new().expect("could not create a temporary directory");
        let path = dunce::canonicalize(root.path()).expect("could not resolve the temporary path");
        let path = Utf8PathBuf::try_from(path).expect("temporary path is not valid UTF-8");

        Self {
            _root: root,
            root: path,
        }
    }

    /// The directory holding the project's `.rover/`.
    pub fn root(&self) -> &Utf8Path {
        &self.root
    }

    /// The project level itself.
    pub fn rover_dir(&self) -> Utf8PathBuf {
        self.root.join(".rover")
    }

    /// Where a project-local plugin binary lands.
    pub fn bin_dir(&self) -> Utf8PathBuf {
        self.rover_dir().join("bin")
    }

    /// Create a directory below the project root and return it, for running a
    /// command somewhere that has to walk up to find the project.
    pub fn subdirectory(&self, relative: impl AsRef<str>) -> Utf8PathBuf {
        let directory = self.root.join(relative.as_ref());
        std::fs::create_dir_all(&directory).expect("could not create the subdirectory");
        directory
    }
}

impl Default for ProjectLevel {
    fn default() -> Self {
        Self::new()
    }
}

/// Which of the two levels a plugin is being placed at, or read from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Global,
    Project,
}

/// Both levels at once, plus the directory a command runs in — the three
/// things that together decide which plugin a run uses.
pub struct TwoLevels {
    pub global: GlobalLevel,
    pub project: ProjectLevel,
    working_dir: Utf8PathBuf,
}

impl TwoLevels {
    pub fn new() -> Self {
        let global = GlobalLevel::new();
        let project = ProjectLevel::new();
        let working_dir = project.root().to_path_buf();

        Self {
            global,
            project,
            working_dir,
        }
    }

    /// Run from a directory below the project root instead of at it, so that
    /// finding the project means walking up.
    pub fn run_from(&mut self, relative: impl AsRef<str>) -> &mut Self {
        self.working_dir = self.project.subdirectory(relative);
        self
    }

    /// The directory a command runs in.
    pub fn working_dir(&self) -> &Utf8Path {
        &self.working_dir
    }

    /// Where a plugin installed at `level` lands.
    pub fn bin_dir(&self, level: Level) -> Utf8PathBuf {
        match level {
            Level::Global => self.global.bin_dir(),
            Level::Project => self.project.bin_dir(),
        }
    }

    /// Put a plugin binary at `level` under the name Rover looks for, and
    /// return where it landed.
    ///
    /// The file is a placeholder: enough for Rover to find it and report its
    /// version, not enough to execute. A test that needs to run a plugin needs
    /// a real one.
    pub fn seed_plugin(&self, level: Level, plugin: &str, version: &str) -> Utf8PathBuf {
        let bin_dir = self.bin_dir(level);
        std::fs::create_dir_all(&bin_dir).expect("could not create the plugin directory");

        // Rover finds plugins by filename: `<name>-v<semver>`, with the
        // platform's executable suffix.
        let binary = bin_dir.join(format!(
            "{plugin}-v{version}{}",
            std::env::consts::EXE_SUFFIX
        ));
        std::fs::write(&binary, b"#!/bin/sh\nexit 0\n").expect("could not write the plugin");

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o755))
                .expect("could not make the plugin executable");
        }

        binary
    }

    /// Put a plugin at `level` that runs, printing `stdout` and exiting 0.
    ///
    /// [`Self::seed_plugin`]'s placeholder is enough for Rover to find a plugin
    /// and name it; this one is enough for Rover to *use* it, so a test can
    /// drive a command past resolution and into what it does with the plugin's
    /// output. A stub means no network and no real composition, at the cost of
    /// a shell script — so tests using it are Unix-only.
    #[cfg(unix)]
    pub fn seed_runnable_plugin(
        &self,
        level: Level,
        plugin: &str,
        version: &str,
        stdout: &str,
    ) -> Utf8PathBuf {
        let binary = self.seed_plugin(level, plugin, version);

        // A quoted heredoc, so nothing in `stdout` is expanded by the shell.
        std::fs::write(
            &binary,
            format!("#!/bin/sh\ncat <<'ROVER_TEST_PLUGIN_EOF'\n{stdout}\nROVER_TEST_PLUGIN_EOF\n"),
        )
        .expect("could not write the plugin");

        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o755))
            .expect("could not make the plugin executable");

        binary
    }

    /// Point a Rover command at both levels, and at the working directory.
    pub fn apply(&self, command: &mut Command) -> &Self {
        self.global.apply(command);
        command.current_dir(&self.working_dir);
        self
    }
}

impl Default for TwoLevels {
    fn default() -> Self {
        Self::new()
    }
}

#[fixture]
pub fn two_levels() -> TwoLevels {
    TwoLevels::new()
}

#[cfg(test)]
mod tests {
    use assert_cmd::cargo::cargo_bin;
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
    fn the_two_levels_are_separate_directories(two_levels: TwoLevels) {
        assert_that!(two_levels.global.rover_dir().is_dir()).is_true();
        assert_that!(two_levels.project.rover_dir().is_dir()).is_true();
        assert_that!(
            two_levels
                .project
                .root()
                .starts_with(two_levels.global.home())
        )
        .is_false();
    }

    #[rstest]
    fn a_command_runs_at_the_project_root_unless_told_otherwise(mut two_levels: TwoLevels) {
        assert_that!(two_levels.working_dir()).is_equal_to(two_levels.project.root());

        let root = two_levels.project.root().to_path_buf();
        two_levels.run_from("services/users");

        assert_that!(two_levels.working_dir()).is_equal_to(root.join("services/users").as_path());
        assert_that!(two_levels.working_dir().is_dir()).is_true();
        // Walking up from there has to still arrive at the project.
        assert_that!(two_levels.project.root()).is_equal_to(root.as_path());
    }

    #[rstest]
    fn a_project_can_be_built_with_no_rover_directory_at_all() {
        let project = ProjectLevel::without_rover_dir();

        assert_that!(project.root().is_dir()).is_true();
        assert_that!(project.rover_dir().exists()).is_false();
    }

    #[rstest]
    fn seeding_puts_a_plugin_in_that_level_and_nowhere_else(
        two_levels: TwoLevels,
        #[values(Level::Global, Level::Project)] level: Level,
    ) {
        let other = match level {
            Level::Global => Level::Project,
            Level::Project => Level::Global,
        };

        let seeded = two_levels.seed_plugin(level, "supergraph", "2.9.0");

        assert_that!(seeded).is_equal_to(
            two_levels
                .bin_dir(level)
                .join(format!("supergraph-v2.9.0{}", std::env::consts::EXE_SUFFIX)),
        );
        assert_that!(seeded.is_file()).is_true();
        assert_that!(two_levels.bin_dir(other).exists()).is_false();
    }

    #[rstest]
    fn rover_looks_for_plugins_in_the_level_this_fixture_builds(two_levels: TwoLevels) {
        // The fixture is only worth anything if Rover agrees with it about
        // where plugins live. `--skip-update` makes Rover look and report,
        // without reaching the network for the plugin itself; `--skip-update-check`
        // is needed too, since it's a separate flag that gates Rover's own
        // self-update check.
        std::fs::write(
            two_levels.working_dir().join("supergraph.yaml"),
            "federation_version: \"2\"\nsubgraphs:\n  users:\n    routing_url: http://localhost:4002\n    schema:\n      file: ./users.graphql\n",
        )
        .expect("could not write the supergraph config");
        std::fs::write(
            two_levels.working_dir().join("users.graphql"),
            "type Query { hello: String }\n",
        )
        .expect("could not write the subgraph schema");

        let mut command = Command::new(cargo_bin("rover"));
        two_levels.apply(&mut command);
        let output = command
            .args([
                "supergraph",
                "compose",
                "--config",
                "supergraph.yaml",
                "--skip-update",
                "--skip-update-check",
            ])
            // Without this, an ambient RUST_BACKTRACE in the environment (some CI
            // runners set it for better diagnostics) makes anyhow append a full
            // stack backtrace to stderr, breaking the exact-match assertion below.
            .env("RUST_BACKTRACE", "0")
            .output()
            .expect("could not run rover");

        // Asserting on the whole error, not a boolean: when this breaks it is
        // usually because Rover stopped somewhere earlier for an unrelated
        // reason, and the message is the only thing that says where. What comes
        // before the error — the progress line, and composition's warning about
        // an unpinned federation_version — belongs to composition rather than to
        // where plugins are looked for, so it is left out rather than pinned here.
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let error = stderr
            .find("error:")
            .map(|start| &stderr[start..])
            .expect("rover printed no error");
        let expected_error = format!(
            "error: Error when updating Federation Version\n\nCaused by:\n    unable to find dependency: \"error: You do not have any 'supergraph' plugins installed in '{bin_dir}'.\n            Re-run this command without the `--skip-update` flag to install the proper plugin.\n    \"\n",
            bin_dir = two_levels.global.bin_dir(),
        );

        assert_that!(error).is_equal_to(expected_error.as_str());
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

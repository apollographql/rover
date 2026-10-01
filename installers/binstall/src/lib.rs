//! Self-installation of `rover`
//!
//! This module contains one public function which will self-install the
//! currently running executable as `Installer::binary_name`. Our goal is to either overwrite
//! the existing installation in `PATH`, or to add a new directory
//! for the binary to live in and add it to `PATH`.
//!
//! On Windows this is intended to be run from PowerShell
//! which is downloaded via iwr | iex.
//!
//! On Unix this is intended to be run from a shell script
//! which is downloaded via curl | sh.
//!
//! Both the PowerShell script and the Unix script download this executable
//! and run it.
//!
//! This may get more complicated over time (self updates anyone?) but for now
//! it's pretty simple! We're largely just moving over our currently running
//! executable to a different path.

use std::convert::TryFrom;

use camino::{Utf8Path, Utf8PathBuf};
use directories_next::BaseDirs;

mod error;
mod install;
mod system;

pub use error::InstallerError;
pub use install::{download, Installer};
#[cfg(not(windows))]
pub(crate) use system::unix;
#[cfg(windows)]
pub(crate) use system::windows;

/// Where an installer keeps `binary_name`'s files: a `.{binary_name}`
/// directory under `override_home` when that is given and not empty, and under
/// `home` otherwise. `None` when neither is usable.
///
/// This is the one rule for Rover's own directory, `~/.rover` or
/// `$APOLLO_HOME/.rover`: self-installs and plugin installs place binaries
/// under it, and anything that needs to find them, or what sits beside them,
/// should ask here rather than join the path itself. An empty override, as an
/// exported but empty `APOLLO_HOME` gives, counts as no override rather than
/// as the working directory.
pub fn base_dir(
    binary_name: &str,
    override_home: Option<&Utf8Path>,
    home: Option<&Utf8Path>,
) -> Option<Utf8PathBuf> {
    let usable = |dir: &&Utf8Path| !dir.as_str().is_empty();
    override_home
        .filter(usable)
        .or_else(|| home.filter(usable))
        .map(|dir| dir.join(format!(".{binary_name}")))
}

/// The user's home directory, the one [`base_dir`] places Rover's own
/// directory under when nothing overrides it.
pub fn get_home_dir_path() -> Result<Utf8PathBuf, InstallerError> {
    match BaseDirs::new() {
        Some(base_dirs) => Ok(Utf8PathBuf::try_from(base_dirs.home_dir().to_path_buf())?),
        None => Err(no_home()),
    }
}

/// The error for a machine with no home directory to install under.
pub(crate) fn no_home() -> InstallerError {
    if cfg!(windows) {
        InstallerError::NoHomeWindows
    } else {
        InstallerError::NoHomeUnix
    }
}

#[cfg(test)]
mod tests {
    #[cfg(not(windows))]
    use std::convert::TryFrom;

    #[cfg(not(windows))]
    use assert_fs::TempDir;
    use camino::Utf8PathBuf;
    use rstest::rstest;
    #[cfg(not(windows))]
    use serial_test::serial;
    use speculoos::prelude::*;

    #[cfg(not(windows))]
    use super::Installer;
    use super::{base_dir, get_home_dir_path};

    #[rstest]
    #[case::the_override_wins(Some("/opt/apollo"), Some("/home/me"), Some("/opt/apollo/.rover"))]
    #[case::home_without_an_override(None, Some("/home/me"), Some("/home/me/.rover"))]
    #[case::an_empty_override_is_no_override(Some(""), Some("/home/me"), Some("/home/me/.rover"))]
    #[case::the_override_without_a_home(Some("/opt/apollo"), None, Some("/opt/apollo/.rover"))]
    #[case::nothing_usable(Some(""), None, None)]
    #[case::an_empty_home_is_no_home(None, Some(""), None)]
    fn base_dir_prefers_a_usable_override_to_home(
        #[case] override_home: Option<&str>,
        #[case] home: Option<&str>,
        #[case] expected: Option<&str>,
    ) {
        let dir = base_dir(
            "rover",
            override_home.map(camino::Utf8Path::new),
            home.map(camino::Utf8Path::new),
        );
        assert_that!(dir).is_equal_to(expected.map(Utf8PathBuf::from));
    }

    #[cfg(not(windows))]
    #[test]
    #[serial]
    fn install_bins_creates_rover_home() {
        let fixture = TempDir::new().unwrap();
        let base_dir = Utf8PathBuf::try_from(fixture.path().to_path_buf()).unwrap();
        let install_path = Installer {
            binary_name: "test".to_string(),
            force_install: false,
            override_install_path: Some(base_dir.clone()),
            executable_location: Utf8PathBuf::try_from(std::env::current_exe().unwrap()).unwrap(),
            install_root: None,
        }
        .install()
        .unwrap()
        .unwrap();

        assert!(install_path.to_string().contains(&base_dir.to_string()));
    }

    #[cfg(not(windows))]
    #[test]
    fn test_get_home_dir_path() {
        let home_dir_path = get_home_dir_path();
        let env_home_dir = std::env::home_dir();
        match env_home_dir {
            Some(home_dir) => {
                let home_dir = Utf8PathBuf::from_path_buf(home_dir)
                    .expect("Unable to convert PathBuf to Utf8PathBuf");
                assert_that!(home_dir_path).is_ok().is_equal_to(home_dir);
            }
            None => {
                assert_that!(home_dir_path)
                    .is_err()
                    .matches(|err| matches!(err, crate::InstallerError::NoHomeUnix));
            }
        }
    }

    #[cfg(windows)]
    #[test]
    fn test_get_home_dir_path() {
        let home_dir_path = get_home_dir_path();
        let env_home_dir = std::env::home_dir();
        match env_home_dir {
            Some(home_dir) => {
                let home_dir = Utf8PathBuf::from_path_buf(home_dir)
                    .expect("Unable to convert PathBuf to Utf8PathBuf");
                assert_that!(home_dir_path).is_ok().is_equal_to(home_dir);
            }
            None => {
                assert_that!(home_dir_path)
                    .is_err()
                    .matches(|err| matches!(err, crate::InstallerError::NoHomeWindows));
            }
        }
    }
}

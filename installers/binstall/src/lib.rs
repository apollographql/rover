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

use camino::Utf8PathBuf;
use directories_next::BaseDirs;

use std::convert::TryFrom;

mod error;
mod install;
mod system;

pub use error::InstallerError;
pub use install::Installer;

#[cfg(not(windows))]
pub(crate) use system::unix;

#[cfg(windows)]
pub(crate) use system::windows;

pub(crate) fn get_home_dir_path() -> Result<Utf8PathBuf, InstallerError> {
    if let Some(base_dirs) = BaseDirs::new() {
        Ok(Utf8PathBuf::try_from(base_dirs.home_dir().to_path_buf())?)
    } else if cfg!(windows) {
        Err(InstallerError::NoHomeWindows)
    } else {
        Err(InstallerError::NoHomeUnix)
    }
}

#[cfg(test)]
mod tests {
    #[cfg(not(windows))]
    use std::convert::TryFrom;

    #[cfg(not(windows))]
    use serial_test::serial;

    #[cfg(not(windows))]
    use super::Installer;

    #[cfg(not(windows))]
    use assert_fs::TempDir;

    #[cfg(not(windows))]
    use camino::Utf8PathBuf;

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
        }
        .install()
        .unwrap()
        .unwrap();

        assert!(install_path.to_string().contains(&base_dir.to_string()));
    }
}

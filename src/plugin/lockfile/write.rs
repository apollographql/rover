//! Writing a `plugin-versions.lock` to disk.
//!
//! This is the one place a lockfile is written, and it asks for a
//! [`LockfileWrite`], which only the `rover plugin` verbs that install or
//! remove plugins can make.

use std::sync::Arc;

use camino::Utf8Path;
use rover_std::Fs;

use super::{PluginLockfile, unusable};
use crate::{
    command::plugin::LockfileWrite,
    plugin::error::{LockfileProblem, PluginFailure},
};

impl PluginLockfile {
    /// Write this lockfile to `path`, replacing whatever is there.
    ///
    /// An interrupted write leaves the old lockfile, not half of the new one.
    /// The directory must already exist: every level has one by the time
    /// something has been installed at it.
    pub fn write(
        &self,
        path: &Utf8Path,
        _permit: &LockfileWrite,
    ) -> Result<(), Box<PluginFailure>> {
        Fs::write_file_atomically(path, self.render())
            .map_err(|source| unusable(path, LockfileProblem::Unwritable(Arc::new(source))))
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use assert_fs::TempDir;
    use camino::Utf8PathBuf;
    use rstest::rstest;
    use semver::Version;
    use speculoos::prelude::*;

    use super::*;
    use crate::plugin::{
        error::printed,
        lockfile::{LOCKFILE, LockedPlugin},
        version::{PluginName, VersionRequest},
    };

    fn temp_dir() -> (TempDir, Utf8PathBuf) {
        let temp = TempDir::new().unwrap();
        let root = Utf8PathBuf::try_from(temp.path().to_path_buf()).unwrap();
        (temp, root)
    }

    fn lockfile(resolved: Version) -> PluginLockfile {
        PluginLockfile::from_iter([LockedPlugin {
            name: PluginName::Supergraph,
            requested: VersionRequest::Major(2),
            resolved,
            checksum: None,
        }])
    }

    #[rstest]
    fn a_written_lockfile_is_exactly_what_it_renders_and_reads_back() {
        let (_temp, root) = temp_dir();
        let path = root.join(LOCKFILE);
        let lockfile = lockfile(Version::new(2, 9, 3));

        lockfile.write(&path, &LockfileWrite::for_tests()).unwrap();

        assert_that!(fs::read_to_string(&path).unwrap()).is_equal_to(lockfile.render());
        assert_that!(PluginLockfile::load(&path))
            .is_ok()
            .is_some()
            .is_equal_to(lockfile);
    }

    #[rstest]
    fn a_lockfile_that_cannot_be_written_names_the_file() {
        let (_temp, root) = temp_dir();
        let path = root.join("missing").join(LOCKFILE);

        let error = lockfile(Version::new(2, 9, 3))
            .write(&path, &LockfileWrite::for_tests())
            .expect_err("should not write");

        assert_that!(printed(error).lines().next().map(str::to_string)).is_equal_to(Some(format!(
            "error[E052]: Couldn't write the plugin lockfile `{path}`."
        )));
        assert_that!(root.join("missing").exists()).is_false();
    }
}

//! Creating a project root, the one thing only `rover plugin install
//! --manifest-path` does (FR25), and the `.gitignore` that goes with it
//! (FR27).

use std::{
    fs,
    io::{self, Write},
};

use anyhow::anyhow;
use camino::{Utf8Path, Utf8PathBuf};

use crate::RoverResult;

/// What a project root's `.gitignore` says: its binaries are not committed,
/// while its manifest and lockfile are.
const GITIGNORE: &str = "bin/\n";

/// A project root an install is creating, because `--manifest-path` named a
/// manifest that doesn't exist yet.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct NewProjectRoot {
    manifest: Utf8PathBuf,
    /// The outermost directory on the way to the root's `bin/` that didn't
    /// exist before this install, if any: what to remove if the install
    /// fails, leaving things as they were.
    outermost_new_dir: Option<Utf8PathBuf>,
}

impl NewProjectRoot {
    /// The root to create for `manifest`, or `None` if it already exists or
    /// is a filesystem root, with no directory to hold it.
    pub(super) fn for_manifest(manifest: &Utf8Path) -> Option<Self> {
        if manifest.symlink_metadata().is_ok() {
            return None;
        }
        // Through the `bin/` the install itself makes.
        let bin = manifest.parent()?.join("bin");
        let outermost_new_dir = bin
            .ancestors()
            .take_while(|dir| !dir.as_str().is_empty() && dir.symlink_metadata().is_err())
            .last()
            .map(Utf8Path::to_path_buf);
        Some(Self {
            manifest: manifest.to_path_buf(),
            outermost_new_dir,
        })
    }

    /// Write the manifest, and a `.gitignore` beside it unless one is there.
    pub(super) fn create(self) -> RoverResult<()> {
        let fail = |err: io::Error, path: &Utf8Path| {
            anyhow!(err).context(format!("Couldn't create `{path}`."))
        };
        if let Some(dir) = self.manifest.parent() {
            fs::create_dir_all(dir).map_err(|err| fail(err, dir))?;
        }
        fs::write(&self.manifest, "").map_err(|err| fail(err, &self.manifest))?;
        let gitignore = self.manifest.with_file_name(".gitignore");
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&gitignore)
        {
            Ok(mut file) => file
                .write_all(GITIGNORE.as_bytes())
                .map_err(|err| fail(err, &gitignore))?,
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => {}
            Err(err) => return Err(fail(err, &gitignore).into()),
        }
        Ok(())
    }

    /// Remove every directory the failed install created on the way to this
    /// root. Nothing that existed before is touched.
    pub(super) fn abandon(self) {
        if let Some(dir) = self.outermost_new_dir {
            let _ = fs::remove_dir_all(dir);
        }
    }
}

#[cfg(test)]
mod tests {
    use assert_fs::TempDir;
    use rstest::rstest;
    use speculoos::prelude::*;

    use super::*;

    fn temp_dir() -> (TempDir, Utf8PathBuf) {
        let temp = TempDir::new().unwrap();
        let root = Utf8PathBuf::try_from(temp.path().to_path_buf()).unwrap();
        (temp, root)
    }

    #[rstest]
    fn an_existing_manifest_needs_no_root() {
        let (_temp, root) = temp_dir();
        fs::write(root.join("rover.yaml"), "").unwrap();

        assert_that!(NewProjectRoot::for_manifest(&root.join("rover.yaml"))).is_equal_to(None);
    }

    #[rstest]
    fn a_root_is_created_with_its_gitignore() {
        let (_temp, root) = temp_dir();
        let manifest = root.join("app/.rover/rover.yaml");

        NewProjectRoot::for_manifest(&manifest)
            .unwrap()
            .create()
            .unwrap();

        assert_that!((
            fs::read_to_string(&manifest).unwrap(),
            fs::read_to_string(root.join("app/.rover/.gitignore")).unwrap(),
        ))
        .is_equal_to((String::new(), GITIGNORE.to_string()));
    }

    #[rstest]
    fn an_existing_gitignore_is_left_alone() {
        let (_temp, root) = temp_dir();
        fs::create_dir(root.join(".rover")).unwrap();
        fs::write(root.join(".rover/.gitignore"), "*.log\n").unwrap();

        NewProjectRoot::for_manifest(&root.join(".rover/rover.yaml"))
            .unwrap()
            .create()
            .unwrap();

        assert_that!(fs::read_to_string(root.join(".rover/.gitignore")).unwrap())
            .is_equal_to("*.log\n".to_string());
    }

    #[rstest]
    #[case::every_directory_new("app/.rover/rover.yaml", "app")]
    #[case::only_the_manifest_new("rover.yaml", "bin")]
    fn abandoning_removes_only_what_was_new(#[case] manifest: &str, #[case] new: &str) {
        let (_temp, root) = temp_dir();
        fs::write(root.join("README.md"), "").unwrap();
        let new_root = NewProjectRoot::for_manifest(&root.join(manifest)).unwrap();
        // What a failed install leaves: the `bin/` it made.
        fs::create_dir_all(root.join(manifest).parent().unwrap().join("bin")).unwrap();

        new_root.abandon();

        let left: Vec<String> = fs::read_dir(&root)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().to_string())
            .collect();
        assert_that!((left, root.join(new).exists()))
            .is_equal_to((vec!["README.md".to_string()], false));
    }
}

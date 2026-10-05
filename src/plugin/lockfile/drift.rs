//! Whether a lockfile still records what the manifest beside it declares.
//!
//! A lockfile only changes on an install or a removal, so hand-editing a
//! manifest leaves the two disagreeing until the next install. When they do,
//! a plugin-using command must fail rather than resolve the declaration
//! afresh, since resolving silently is what the lockfile exists to prevent.
//!
//! An absent lockfile is not drift. A manifest that has never been installed
//! from resolves exactly as though there were no manifest at all.

use camino::Utf8Path;

use super::{LOCKFILE, LockedPlugin, PluginLockfile};
use crate::plugin::{
    error::PluginFailure,
    manifest::{MANIFEST_FILE, RoverManifest},
    version::VersionRequest,
};

/// Check that the lockfile in a level's `.rover/` directory `dir` records
/// a release of every plugin the manifest there declares that the
/// declaration allows. A level missing either file cannot drift.
///
/// The fix a drift suggests is a `rover plugin install`, which updates only
/// the global lockfile until a project can install its own plugins; until
/// then, it does not fix drift at the project level.
pub fn check_drift(dir: &Utf8Path) -> Result<(), Box<PluginFailure>> {
    let manifest_path = dir.join(MANIFEST_FILE);
    let Some(manifest) = RoverManifest::load(&manifest_path)? else {
        return Ok(());
    };
    let Some(lockfile) = PluginLockfile::load(&dir.join(LOCKFILE))? else {
        return Ok(());
    };

    lockfile.check_against(&manifest_path, &manifest)
}

impl PluginLockfile {
    /// Check that this lockfile records every plugin `manifest`, read from
    /// `manifest_path`, declares, naming the first that it doesn't.
    pub fn check_against(
        &self,
        manifest_path: &Utf8Path,
        manifest: &RoverManifest,
    ) -> Result<(), Box<PluginFailure>> {
        self.first_drift(manifest_path, manifest, true)
    }

    /// [`Self::check_against`], except that a declaration this lockfile has
    /// no entry for yet has not drifted: installing from the manifest is
    /// what resolves and records it. Only a locked release the declaration
    /// no longer allows has.
    pub fn check_entries_against(
        &self,
        manifest_path: &Utf8Path,
        manifest: &RoverManifest,
    ) -> Result<(), Box<PluginFailure>> {
        self.first_drift(manifest_path, manifest, false)
    }

    fn first_drift(
        &self,
        manifest_path: &Utf8Path,
        manifest: &RoverManifest,
        unlocked_has_drifted: bool,
    ) -> Result<(), Box<PluginFailure>> {
        let drifted = manifest.plugins.iter().find_map(|(plugin, declaration)| {
            let locked = self.get(plugin);
            let agrees = locked.map_or(!unlocked_has_drifted, |locked| {
                records(locked, &declaration.request)
            });
            (!agrees).then(|| PluginFailure::LockfileDrift {
                manifest: manifest_path.to_path_buf(),
                plugin,
                declared: declaration.request.clone(),
                locked: locked.cloned(),
            })
        });

        drifted.map_or(Ok(()), |failure| Err(Box::new(failure)))
    }
}

/// Whether `locked` is a release `declared` allows: that release, for an
/// exact declaration, and one in the declared major for a major, however
/// either came to be locked. `latest` allows whatever is locked, since the
/// lock's job is to pin a floating request until it is re-resolved.
fn records(locked: &LockedPlugin, declared: &VersionRequest) -> bool {
    match (declared.exact(), declared.major()) {
        (Some(exact), _) => locked.resolved == *exact,
        (None, Some(major)) => locked.resolved.major == major,
        (None, None) => true,
    }
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

    const LOCKED: &str = indoc! {r#"
        version = 1

        [[plugins]]
        name = "supergraph"
        requested = "2"
        resolved = "2.9.3"

        [[plugins]]
        name = "router"
        requested = "latest"
        resolved = "2.1.0"
    "#};

    fn temp_dir() -> (TempDir, Utf8PathBuf) {
        let temp = TempDir::new().unwrap();
        let root = Utf8PathBuf::try_from(temp.path().to_path_buf()).unwrap();
        (temp, root)
    }

    fn check(manifest: &str) -> Result<(), String> {
        check_with(manifest, PluginLockfile::check_against)
    }

    fn check_with(
        manifest: &str,
        check: fn(&PluginLockfile, &Utf8Path, &RoverManifest) -> Result<(), Box<PluginFailure>>,
    ) -> Result<(), String> {
        let lockfile = PluginLockfile::parse(Utf8Path::new(LOCKFILE), LOCKED).unwrap();
        let manifest = RoverManifest::parse(Utf8Path::new(MANIFEST_FILE), manifest).unwrap();

        check(&lockfile, Utf8Path::new(MANIFEST_FILE), &manifest)
            .map_err(|failure| failure.to_string())
    }

    #[rstest]
    #[case::the_same_floating_requests("plugins:\n  supergraph: \"2\"\n  router: latest\n")]
    #[case::an_exact_declaration_of_the_locked_release("plugins:\n  router: \"=2.1.0\"\n")]
    #[case::a_legacy_spelling_of_the_locked_request("plugins:\n  supergraph: latest-2\n")]
    #[case::latest_whatever_was_locked("plugins:\n  supergraph: latest\n  router: latest\n")]
    #[case::the_locked_major_from_another_request("plugins:\n  router: \"2\"\n")]
    #[case::fewer_declarations_than_entries("plugins:\n  supergraph: \"2\"\n")]
    #[case::no_declarations("")]
    fn a_lockfile_recording_every_declaration_is_in_sync(#[case] manifest: &str) {
        assert_that!(check(manifest)).is_ok();
    }

    #[rstest]
    #[case::a_declaration_the_lockfile_lacks(
        "plugins:\n  apollo-mcp-server: latest\n",
        "`apollo-mcp-server` is declared as `latest` but isn't locked."
    )]
    #[case::an_exact_declaration_of_another_release(
        "plugins:\n  router: \"=2.2.0\"\n",
        "`router` is declared as `=2.2.0` but locked at `2.1.0`."
    )]
    #[case::a_major_the_locked_release_is_outside(
        "plugins:\n  router: \"1\"\n",
        "`router` is declared as `1` but locked at `2.1.0`."
    )]
    #[case::the_first_disagreement_in_plugin_order(
        "plugins:\n  router: \"=2.2.0\"\n  supergraph: \"=2.9.0\"\n",
        "`supergraph` is declared as `=2.9.0` but locked at `2.9.3`."
    )]
    fn a_disagreement_names_the_plugin_and_both_sides(
        #[case] manifest: &str,
        #[case] expected: &str,
    ) {
        assert_that!(check(manifest)).is_equal_to(Err(format!(
            "The plugin lockfile is out of date with `{MANIFEST_FILE}`: {expected}"
        )));
    }

    #[rstest]
    #[case::a_declaration_the_lockfile_lacks("plugins:\n  apollo-mcp-server: latest\n", Ok(()))]
    #[case::a_locked_release_the_declaration_allows("plugins:\n  router: \"2\"\n", Ok(()))]
    #[case::a_locked_release_the_declaration_no_longer_allows(
        "plugins:\n  apollo-mcp-server: latest\n  router: \"=2.2.0\"\n",
        Err(format!(
            "The plugin lockfile is out of date with `{MANIFEST_FILE}`: `router` is declared as \
             `=2.2.0` but locked at `2.1.0`."
        ))
    )]
    fn checking_only_entries_ignores_a_declaration_not_locked_yet(
        #[case] manifest: &str,
        #[case] expected: Result<(), String>,
    ) {
        assert_that!(check_with(manifest, PluginLockfile::check_entries_against))
            .is_equal_to(expected);
    }

    #[rstest]
    fn a_manifest_with_no_lockfile_beside_it_has_not_drifted() {
        let (_temp, root) = temp_dir();
        fs::write(root.join(MANIFEST_FILE), "plugins:\n  router: \"=2.2.0\"\n").unwrap();

        assert_that!(check_drift(&root)).is_ok();
    }

    #[rstest]
    fn a_lockfile_with_no_manifest_beside_it_has_not_drifted() {
        let (_temp, root) = temp_dir();
        fs::write(root.join(LOCKFILE), LOCKED).unwrap();

        assert_that!(check_drift(&root)).is_ok();
    }

    #[rstest]
    fn a_level_whose_files_disagree_names_its_manifest() {
        let (_temp, root) = temp_dir();
        fs::write(root.join(MANIFEST_FILE), "plugins:\n  router: \"=2.2.0\"\n").unwrap();
        fs::write(root.join(LOCKFILE), LOCKED).unwrap();

        let failure = check_drift(&root).expect_err("should have drifted");

        assert_that!(failure.to_string()).is_equal_to(format!(
            "The plugin lockfile is out of date with `{}`: `router` is declared as `=2.2.0` but \
             locked at `2.1.0`.",
            root.join(MANIFEST_FILE)
        ));
    }

    #[rstest]
    fn an_unusable_lockfile_is_reported_rather_than_skipped() {
        let (_temp, root) = temp_dir();
        fs::write(root.join(MANIFEST_FILE), "plugins:\n  router: latest\n").unwrap();
        fs::write(root.join(LOCKFILE), "version = 9\n").unwrap();

        let failure = check_drift(&root).expect_err("should not check");

        assert_that!(failure.to_string()).is_equal_to(format!(
            "`{}` was written by a newer version of Rover, in lockfile format version 9.",
            root.join(LOCKFILE)
        ));
    }
}

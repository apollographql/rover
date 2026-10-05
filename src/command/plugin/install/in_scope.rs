//! What a bare `rover plugin install` installs: the plugins the manifest and
//! lockfile at the level it targets declare, the way `npm ci` installs from
//! `package-lock.json`.

use anyhow::anyhow;
use camino::{Utf8Path, Utf8PathBuf};

use crate::{
    RoverError, RoverErrorSuggestion, RoverResult,
    plugin::{
        discovery::ManifestDirs,
        error::PluginFailure,
        layering::{DeclarationLevel, LayeredDeclarations},
        lockfile::{LOCKFILE, PluginLockfile},
        manifest::{MANIFEST_FILE, RoverManifest},
        precedence::{self, PluginRequest, RequestInputs, RequestSource},
        version::{PluginName, VersionRequest},
    },
};

/// The manifest and lockfile at the level an install targets.
pub(super) struct InScope {
    /// The level's directory, holding its lockfile.
    dir: Utf8PathBuf,
    level: DeclarationLevel,
    manifest_path: Utf8PathBuf,
    manifest: Option<RoverManifest>,
    lockfile: Option<PluginLockfile>,
}

impl InScope {
    /// Read the level in `dir`: a project whose manifest is
    /// `project_manifest`, or the global level when there is none.
    pub(super) fn load(dir: Utf8PathBuf, project_manifest: Option<&Utf8Path>) -> RoverResult<Self> {
        let lockfile = PluginLockfile::load(&dir.join(LOCKFILE))?;
        let manifest_path =
            project_manifest.map_or_else(|| dir.join(MANIFEST_FILE), Utf8Path::to_path_buf);
        let manifest = RoverManifest::load(&manifest_path)?;
        let level = match project_manifest {
            Some(_) => DeclarationLevel::Project,
            None => DeclarationLevel::Global,
        };
        Ok(Self {
            dir,
            level,
            manifest_path,
            manifest,
            lockfile,
        })
    }

    /// Every plugin this level's lockfile records, at exactly the release it
    /// records (FR36), and every plugin its manifest declares that the
    /// lockfile doesn't, to be resolved afresh; each with whether the
    /// lockfile lacks it, and so whether to record what it installs. Only
    /// those are recorded, so no other entry is re-resolved (FR16). A locked
    /// release the manifest no longer allows fails rather than be replaced
    /// (FR18). With neither file there is nothing to install; a file that
    /// names no plugin installs nothing.
    pub(super) fn requests(&self) -> RoverResult<Vec<(PluginRequest, bool)>> {
        if self.manifest.is_none() && self.lockfile.is_none() {
            return Err(nothing_to_install(Some(self)));
        }
        let lockfile = self.lockfile.clone().unwrap_or_default();
        if let Some(manifest) = &self.manifest {
            lockfile.check_entries_against(&self.manifest_path, manifest)?;
        }
        let here = |level| (self.level == level).then(|| self.dir.clone());
        let dirs = ManifestDirs {
            global: here(DeclarationLevel::Global),
            project: here(DeclarationLevel::Project),
        };
        let declarations = match self.level {
            DeclarationLevel::Project => LayeredDeclarations::new(None, self.manifest.as_ref()),
            DeclarationLevel::Global => LayeredDeclarations::new(self.manifest.as_ref(), None),
        };

        let requests = PluginName::ALL.into_iter().filter_map(|plugin| {
            let locked = lockfile.get(plugin);
            let request = match (declarations.get(plugin), locked) {
                // Pinned to the lockfile exactly as every plugin-using
                // command pins a floating declaration (FR31).
                (Some(declared), _) => precedence::request(
                    plugin,
                    RequestInputs::new(declared.declaration.request.clone()),
                    &dirs,
                    &declarations,
                ),
                (None, Some(locked)) => Ok(PluginRequest {
                    plugin,
                    request: VersionRequest::Exact(locked.resolved.clone()),
                    source: RequestSource::Lockfile(self.level),
                }),
                (None, None) => return None,
            };
            Some(request.map(|request| (request, locked.is_none())))
        });
        Ok(requests.collect::<Result<_, Box<PluginFailure>>>()?)
    }
}

/// Why a bare `rover plugin install` has nothing to install: the level it
/// targets, if it has one, has neither a manifest nor a lockfile.
pub(super) fn nothing_to_install(in_scope: Option<&InScope>) -> RoverError {
    let missing = in_scope.map_or_else(
        || format!("there's no `{MANIFEST_FILE}` or `{LOCKFILE}` in scope"),
        |in_scope| {
            format!(
                "there's no `{}` or `{}`",
                in_scope.manifest_path,
                in_scope.dir.join(LOCKFILE)
            )
        },
    );
    let mut err = RoverError::new(anyhow!(
        "No plugin was named, and {missing} to install from."
    ));
    err.set_suggestion(RoverErrorSuggestion::Adhoc(
        "Name the plugin to install, as in `rover plugin install supergraph@2`. Valid plugins \
         are `supergraph`, `router`, and `apollo-mcp-server`."
            .to_string(),
    ));
    err
}

#[cfg(test)]
mod tests {
    use std::fs;

    use assert_fs::TempDir;
    use rstest::rstest;
    use semver::Version;
    use speculoos::prelude::*;

    use super::*;

    const LOCKED: &str = "version = 1\n\n[[plugins]]\nname = \"supergraph\"\nrequested = \
                          \"2\"\nresolved = \"2.9.3\"\n\n[[plugins]]\nname = \"router\"\n\
                          requested = \"latest\"\nresolved = \"2.1.0\"\n";

    /// The requests a project holding `manifest` and `lockfile`, where
    /// given, installs, as each plugin's request, its source, and whether
    /// it is recorded.
    fn requests(
        manifest: Option<&str>,
        lockfile: Option<&str>,
    ) -> Vec<(PluginName, VersionRequest, RequestSource, bool)> {
        let temp = TempDir::new().unwrap();
        let dir = Utf8PathBuf::try_from(temp.path().to_path_buf()).unwrap();
        if let Some(manifest) = manifest {
            fs::write(dir.join(MANIFEST_FILE), manifest).unwrap();
        }
        if let Some(lockfile) = lockfile {
            fs::write(dir.join(LOCKFILE), lockfile).unwrap();
        }
        let manifest_path = dir.join(MANIFEST_FILE);

        InScope::load(dir, Some(&manifest_path))
            .unwrap()
            .requests()
            .unwrap()
            .into_iter()
            .map(|(request, record)| (request.plugin, request.request, request.source, record))
            .collect()
    }

    fn exact(major: u64, minor: u64, patch: u64) -> VersionRequest {
        VersionRequest::Exact(Version::new(major, minor, patch))
    }

    const LOCKED_HERE: RequestSource = RequestSource::Lockfile(DeclarationLevel::Project);

    #[rstest]
    fn every_locked_plugin_installs_at_its_locked_release_and_is_not_recorded_again() {
        assert_that!(requests(None, Some(LOCKED))).is_equal_to(vec![
            (PluginName::Supergraph, exact(2, 9, 3), LOCKED_HERE, false),
            (PluginName::Router, exact(2, 1, 0), LOCKED_HERE, false),
        ]);
    }

    #[rstest]
    fn only_a_declaration_the_lockfile_lacks_is_resolved_and_recorded() {
        let manifest = "plugins:\n  supergraph: \"2\"\n  router: \"=2.1.0\"\n  \
                        apollo-mcp-server: latest\n";

        assert_that!(requests(Some(manifest), Some(LOCKED))).is_equal_to(vec![
            (PluginName::Supergraph, exact(2, 9, 3), LOCKED_HERE, false),
            (
                PluginName::Router,
                exact(2, 1, 0),
                RequestSource::Manifest(DeclarationLevel::Project),
                false,
            ),
            (
                PluginName::ApolloMcpServer,
                VersionRequest::Latest,
                RequestSource::Manifest(DeclarationLevel::Project),
                true,
            ),
        ]);
    }

    #[rstest]
    fn a_manifest_with_no_lockfile_resolves_and_records_every_declaration() {
        assert_that!(requests(Some("plugins:\n  supergraph: \"2\"\n"), None)).is_equal_to(vec![(
            PluginName::Supergraph,
            VersionRequest::Major(2),
            RequestSource::Manifest(DeclarationLevel::Project),
            true,
        )]);
    }
}

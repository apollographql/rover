//! What a bare `rover plugin install` installs: the plugins the lockfile at
//! the level it targets records, the way `npm ci` installs from
//! `package-lock.json`.

use anyhow::anyhow;
use camino::{Utf8Path, Utf8PathBuf};

use crate::{
    RoverError, RoverErrorSuggestion, RoverResult,
    plugin::{
        layering::DeclarationLevel,
        lockfile::{LOCKFILE, PluginLockfile},
        manifest::{MANIFEST_FILE, RoverManifest},
        precedence::{PluginRequest, RequestSource},
        version::VersionRequest,
    },
};

/// The lockfile at the level an install targets.
pub(super) struct InScope {
    /// The level's directory, holding its lockfile.
    dir: Utf8PathBuf,
    level: DeclarationLevel,
    lockfile: Option<PluginLockfile>,
}

impl InScope {
    /// Read the level in `dir`: a project whose manifest is
    /// `project_manifest`, or the global level when there is none. The
    /// manifest is read too, so that one this can't honor stops the install.
    pub(super) fn load(dir: Utf8PathBuf, project_manifest: Option<&Utf8Path>) -> RoverResult<Self> {
        let lockfile = PluginLockfile::load(&dir.join(LOCKFILE))?;
        RoverManifest::load(
            &project_manifest.map_or_else(|| dir.join(MANIFEST_FILE), Utf8Path::to_path_buf),
        )?;
        let level = match project_manifest {
            Some(_) => DeclarationLevel::Project,
            None => DeclarationLevel::Global,
        };
        Ok(Self {
            dir,
            level,
            lockfile,
        })
    }

    /// Every plugin this level's lockfile records, at exactly the release it
    /// records, so that nothing needs resolving (FR36). With no lockfile
    /// there is nothing to install; one that records no plugin installs
    /// nothing.
    pub(super) fn requests(&self) -> RoverResult<Vec<PluginRequest>> {
        let lockfile = self
            .lockfile
            .as_ref()
            .ok_or_else(|| nothing_to_install(Some(self)))?;
        Ok(lockfile
            .iter()
            .map(|locked| PluginRequest {
                plugin: locked.name,
                request: VersionRequest::Exact(locked.resolved.clone()),
                source: RequestSource::Lockfile(self.level),
            })
            .collect())
    }
}

/// Why a bare `rover plugin install` has nothing to install: the level it
/// targets, if it has one, has no lockfile.
pub(super) fn nothing_to_install(in_scope: Option<&InScope>) -> RoverError {
    let missing = in_scope.map_or_else(
        || format!("there's no `{LOCKFILE}` in scope"),
        |in_scope| format!("there's no `{}`", in_scope.dir.join(LOCKFILE)),
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

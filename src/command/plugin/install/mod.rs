mod in_scope;
mod output;
mod project_root;

use anyhow::anyhow;
use binstall::Installer;
use camino::{Utf8Path, Utf8PathBuf};
use clap::Parser;
use rover_print::{print::Print, style::StyledText};
use serde::Serialize;

use self::{
    in_scope::{InScope, nothing_to_install},
    output::PluginInstallOutput,
    project_root::NewProjectRoot,
};
use super::LockfileWrite;
use crate::{
    PKG_NAME, RoverError, RoverErrorSuggestion, RoverOutput, RoverResult,
    command::install::{Plugin, PluginInstaller, PluginProvenance, PluginSource, installer},
    options::LicenseAccepter,
    plugin::{
        discovery::{ManifestDirs, project_in_scope},
        error::{DownloadControl, RequestOrigin},
        layering::LayeredDeclarations,
        lockfile::{LOCKFILE, LockedPlugin, PluginLockfile},
        manifest::MANIFEST_FILE,
        precedence::{self, PluginRequest, RequestInputs},
    },
    utils::{GLOBAL_ENV, client::StudioClientConfig},
};

#[derive(Debug, Serialize, Parser)]
pub struct PluginInstall {
    /// The plugin to install, as `<NAME>@<VERSION>`, e.g. `supergraph@=2.9.3` or `router@2`.
    /// Leave it out to install every plugin in scope
    ///
    /// Without it, every plugin the lockfile in scope records is installed at
    /// exactly the version it records, without asking the plugin registry,
    /// and every plugin the manifest declares that the lockfile doesn't
    /// record yet is installed and recorded too.
    #[arg(value_name = "NAME@VERSION")]
    pub(crate) plugin: Option<Plugin>,

    /// Overwrite any existing binary without prompting for confirmation.
    #[arg(long = "force", short = 'f')]
    pub(crate) force: bool,

    /// Never download: use the plugin if it's already installed, and fail,
    /// naming it, if it isn't.
    ///
    /// Set the `APOLLO_ROVER_NO_DOWNLOAD` environment variable (to `1` or `true`)
    /// to do the same. Neither this nor `--skip-update` implies the other.
    #[arg(long = "no-download")]
    pub(crate) no_download: bool,

    /// Install into the global install root, shared by every project, even
    /// inside a project that has its own.
    ///
    /// Set the `APOLLO_ROVER_GLOBAL` environment variable (to `1` or `true`)
    /// to do the same.
    #[arg(long = "global", short = 'g')]
    pub(crate) global: bool,

    /// Install into the project whose manifest this is, rather than the one
    /// found by searching up from the working directory.
    ///
    /// The project's plugins go in a `bin/` directory beside the manifest, and
    /// its lockfile is `plugin-versions.lock` there too. A manifest that doesn't
    /// exist yet is created, along with the rest of the project and a
    /// `.gitignore` that keeps `bin/` out of version control.
    #[arg(
        long = "manifest-path",
        short = 'm',
        value_name = "FILE",
        conflicts_with = "global"
    )]
    pub(crate) manifest_path: Option<Utf8PathBuf>,

    #[clap(flatten)]
    pub(crate) elv2_license_accepter: LicenseAccepter,
}

impl PluginInstall {
    pub async fn run(
        &self,
        override_install_path: Option<Utf8PathBuf>,
        client_config: StudioClientConfig,
    ) -> RoverResult<RoverOutput> {
        let project_manifest = self.project_manifest(override_install_path.as_deref())?;
        let make_installer = || -> RoverResult<Installer> {
            let mut rover_installer =
                installer(PKG_NAME, self.force, override_install_path.clone())?;
            rover_installer.install_root = project_manifest
                .as_deref()
                .and_then(Utf8Path::parent)
                .map(Utf8Path::to_path_buf);
            Ok(rover_installer)
        };
        let rover_installer = make_installer()?;
        // Only a named manifest makes a project root where there was none.
        let new_root = project_manifest
            .as_deref()
            .filter(|_| self.manifest_path.is_some())
            .and_then(NewProjectRoot::for_manifest);
        let level = recording_level(&rover_installer)?;
        // Read before installing, so that a lockfile this can't update
        // stops the install rather than leaving a plugin it doesn't record,
        // and a manifest asking for a redirected install root stops it
        // rather than see the plugin put somewhere it didn't ask for. An
        // install into `node_modules/.bin` is no level's and records nothing,
        // but a bare one still installs from the global level, and fails as
        // a named install does when there is no home to find it in.
        let read_from = match (&level, &self.plugin) {
            (Some(level), _) => Some(level.clone()),
            (None, None) => Some(rover_installer.get_base_dir_path()?),
            (None, Some(_)) => None,
        };
        let in_scope = read_from
            .map(|dir| InScope::load(dir, project_manifest.as_deref()))
            .transpose()?;

        // Each request, with whether to record what it installs.
        let installs = match &self.plugin {
            // A plugin named on the command line is the most specific source
            // on the ladder every plugin-using command shares, so nothing
            // below it is consulted. It is never pinned by a lockfile
            // either: a plugin named on the command line is resolved afresh.
            Some(named) => {
                let request = precedence::resolve(
                    named.name(),
                    RequestInputs::new(named.request())
                        .with_override(Some((named.request(), RequestOrigin::PluginArgument))),
                    &LayeredDeclarations::default(),
                );
                vec![(request, true)]
            }
            None => in_scope
                .as_ref()
                .ok_or_else(|| nothing_to_install(None))?
                .requests()?,
        };
        let plugins = installs
            .iter()
            .map(|(request, _)| Plugin::from_request(request.plugin, &request.request))
            .collect::<Result<Vec<_>, _>>()?;
        // Whatever the requests came from: the argument, the manifest, or the lockfile. Before
        // the license prompt, so nothing is asked for or downloaded on the way to refusing.
        for plugin in &plugins {
            plugin.reject_unsupported()?;
        }
        if plugins.iter().any(Plugin::requires_elv2_license) {
            self.elv2_license_accepter
                .require_elv2_license(&client_config)?;
        }

        let installed = async {
            let mut installed = Vec::new();
            for ((request, record), plugin) in installs.iter().zip(&plugins) {
                let provenance =
                    PluginInstaller::new(client_config.clone(), make_installer()?, self.force)
                        .requested_by(request.origin())
                        .without_downloads(self.download_control())
                        .install(plugin)
                        .await?;
                if *record && let Some(level) = &level {
                    self.record(level, request, &provenance)?;
                }
                installed.push(provenance);
            }
            Ok(installed)
        }
        .await;
        // A failed install leaves no half-made project root behind.
        let installed = match (installed, new_root) {
            (Ok(installed), Some(new_root)) => {
                new_root.create()?;
                installed
            }
            (Ok(installed), None) => installed,
            (Err(err), new_root) => {
                if let Some(new_root) = new_root {
                    new_root.abandon();
                }
                return Err(err);
            }
        };

        // A download has already reported itself; anything else would otherwise succeed in
        // silence, which reads as though nothing happened.
        let stderr = rover_print::print::stderr::default();
        for plugin in &installed {
            if let Some(message) = plugin.install_confirmation() {
                stderr.print(&StyledText::plain(message));
            }
        }

        Ok(RoverOutput::CliOutput(Box::new(PluginInstallOutput {
            plugins: installed,
        })))
    }

    /// Record in `level`'s lockfile that `request` installed `installed`,
    /// leaving every other plugin's entry as it was.
    fn record(
        &self,
        level: &Utf8Path,
        request: &PluginRequest,
        installed: &PluginProvenance,
    ) -> RoverResult<()> {
        // With downloads disabled, a floating request was resolved against
        // nothing: the newest release on disk is not what it resolves to, and
        // locking it would say so. An exact request needs no resolving.
        let resolved = self.download_control().is_none() || !request.request.is_floating();
        if !resolved || !belongs_in_lockfile(installed, level) {
            return Ok(());
        }
        let path = level.join(LOCKFILE);
        // Read again rather than reuse the first read: another install may
        // have recorded a plugin while this one downloaded.
        PluginLockfile::load(&path)?
            .unwrap_or_default()
            .with(LockedPlugin {
                name: request.plugin,
                requested: request.request.clone(),
                resolved: installed.version.clone(),
                checksum: None,
            })
            .write(&path, &LockfileWrite(()))?;
        Ok(())
    }

    /// The manifest of the project this installs into, or `None` to install
    /// globally. The project's install root is the directory holding it.
    ///
    /// That is the `--manifest-path` manifest when one is named (FR25), none
    /// when a global install is asked for (FR24), and otherwise the manifest
    /// of the project in scope, if there is one (FR23).
    fn project_manifest(&self, apollo_home: Option<&Utf8Path>) -> RoverResult<Option<Utf8PathBuf>> {
        match (&self.manifest_path, self.global()) {
            // Clap refuses the two flags together, so only the variable
            // gets here.
            (Some(_), true) => {
                let mut err = RoverError::new(anyhow!(
                    "`--manifest-path` names a project to install into, but `{GLOBAL_ENV}` \
                     asks for a global install."
                ));
                err.set_suggestion(RoverErrorSuggestion::Adhoc(format!(
                    "Unset `{GLOBAL_ENV}` to install into the project, or drop \
                     `--manifest-path` to install globally."
                )));
                Err(err)
            }
            (Some(manifest), false) => absolute_manifest(manifest).map(Some),
            (None, true) => Ok(None),
            (None, false) => Ok(
                project_in_scope(ManifestDirs::for_this_process(apollo_home))
                    .map(|project| project.join(MANIFEST_FILE)),
            ),
        }
    }

    /// The manifest `--manifest-path` names, made absolute against the working
    /// directory, or `None` without the flag. The one place that happens, so
    /// the install and settings resolution (FR78 of the profile-configuration
    /// spec) always mean the same file.
    pub(crate) fn named_manifest(&self) -> Option<RoverResult<Utf8PathBuf>> {
        self.manifest_path.as_deref().map(absolute_manifest)
    }

    /// Whether this install goes to the global level whatever project is in
    /// scope: by the flag, or its environment variable.
    fn global(&self) -> bool {
        self.global || crate::utils::global_install()
    }

    /// What forbids this install from downloading, if anything does: the
    /// flag, or failing that, its environment variable.
    fn download_control(&self) -> Option<DownloadControl> {
        if self.no_download {
            Some(DownloadControl::NoDownloadFlag)
        } else if crate::utils::no_download() {
            Some(DownloadControl::NoDownloadEnvVar)
        } else {
            None
        }
    }
}

/// The level whose lockfile an install through `installer` records, or `None`
/// when plugins are going somewhere that is no level's.
///
/// That is the installer's install root when it has one, which is the project
/// in scope, and otherwise the global level, unless
/// `APOLLO_NODE_MODULES_BIN_DIR` sends plugins into an npm package's
/// `node_modules/.bin`. An install there needs no home directory, so none is
/// looked for, and no lockfile is read.
fn recording_level(installer: &Installer) -> RoverResult<Option<Utf8PathBuf>> {
    let bin_dir = installer.bin_dir_location()?;
    if bin_dir.file_name() != Some("bin") {
        return Ok(None);
    }
    let level = installer.get_base_dir_path()?;
    Ok((bin_dir.parent() == Some(&level)).then_some(level))
}

/// Whether `installed` belongs in `level`'s lockfile.
///
/// Only a plugin in the level's own `bin` directory does.
/// `APOLLO_NODE_MODULES_BIN_DIR` sends plugins into an npm package's
/// `node_modules/.bin` instead, which is no level's, and which the global
/// lockfile must not claim to record. Nor does a fallback to a release already
/// installed when the registry couldn't be reached: nothing was resolved, and
/// locking it would pass off a stale release as what the request resolves to.
fn belongs_in_lockfile(installed: &PluginProvenance, level: &Utf8Path) -> bool {
    installed.source != PluginSource::Fallback
        && installed.path.parent() == Some(&level.join("bin"))
}

/// `manifest`, made absolute against the working directory.
fn absolute_manifest(manifest: &Utf8Path) -> RoverResult<Utf8PathBuf> {
    let absolute = std::path::absolute(manifest)
        .map_err(|err| anyhow!(err).context(format!("Couldn't find `{manifest}`.")))?;
    Ok(Utf8PathBuf::try_from(absolute).map_err(|err| anyhow!(err))?)
}

#[cfg(test)]
mod tests {
    use camino::Utf8Path;
    use rstest::rstest;
    use semver::Version;
    use speculoos::prelude::*;

    use super::*;
    use crate::command::install::PluginLevel;

    const IN_BIN: &str = "/home/me/.rover/bin/supergraph-v2.9.3";

    /// A relative `--manifest-path` is resolved against the working directory,
    /// and an absolute one is kept as it is - the one resolution the install
    /// and settings resolution share.
    #[rstest]
    #[case::relative("ci/.rover/rover-ci.yaml")]
    #[case::absolute("/srv/app/.rover/rover.yaml")]
    fn a_named_manifest_is_made_absolute(#[case] named: &str) {
        let resolved = absolute_manifest(Utf8Path::new(named)).unwrap();

        let expected = if Utf8Path::new(named).is_absolute() {
            Utf8PathBuf::from(named)
        } else {
            Utf8PathBuf::try_from(std::env::current_dir().unwrap())
                .unwrap()
                .join(named)
        };
        assert_that!(resolved).is_equal_to(expected);
    }

    #[rstest]
    #[case::downloaded_into_the_levels_bin_directory(IN_BIN, PluginSource::Downloaded, true)]
    #[case::already_installed_there(IN_BIN, PluginSource::Installed, true)]
    #[case::a_fallback_to_an_installed_release(IN_BIN, PluginSource::Fallback, false)]
    #[case::node_modules(
        "/work/app/node_modules/.bin/supergraph-v2.9.3",
        PluginSource::Downloaded,
        false
    )]
    #[case::another_level(
        "/elsewhere/.rover/bin/supergraph-v2.9.3",
        PluginSource::Downloaded,
        false
    )]
    #[case::below_the_bin_directory(
        "/home/me/.rover/bin/old/supergraph-v2.9.3",
        PluginSource::Downloaded,
        false
    )]
    fn only_a_plugin_installed_into_the_level_is_recorded(
        #[case] path: &str,
        #[case] source: PluginSource,
        #[case] recorded: bool,
    ) {
        let installed = PluginProvenance::new(
            "supergraph",
            Version::new(2, 9, 3),
            source,
            PluginLevel::Global,
            path.into(),
        );

        assert_that!(belongs_in_lockfile(
            &installed,
            Utf8Path::new("/home/me/.rover")
        ))
        .is_equal_to(recorded);
    }
}

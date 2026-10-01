mod output;

use binstall::Installer;
use camino::{Utf8Path, Utf8PathBuf};
use clap::Parser;
use serde::Serialize;

use self::output::PluginInstallOutput;
use super::LockfileWrite;
use crate::{
    PKG_NAME, RoverOutput, RoverResult,
    command::install::{Plugin, PluginInstaller, PluginProvenance, PluginSource, installer},
    options::LicenseAccepter,
    plugin::{
        error::{DownloadControl, RequestOrigin},
        lockfile::{LOCKFILE, LockedPlugin, PluginLockfile},
    },
    utils::client::StudioClientConfig,
};

#[derive(Debug, Serialize, Parser)]
pub struct PluginInstall {
    /// The plugin to install, as `<NAME>@<VERSION>`, e.g. `supergraph@=2.9.3` or `router@2`
    #[arg(value_name = "NAME@VERSION")]
    pub(crate) plugin: Plugin,

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

    #[clap(flatten)]
    pub(crate) elv2_license_accepter: LicenseAccepter,
}

impl PluginInstall {
    pub async fn run(
        &self,
        override_install_path: Option<Utf8PathBuf>,
        client_config: StudioClientConfig,
    ) -> RoverResult<RoverOutput> {
        if self.plugin.requires_elv2_license() {
            self.elv2_license_accepter
                .require_elv2_license(&client_config)?;
        }
        let rover_installer = installer(PKG_NAME, self.force, override_install_path)?;
        let level = recording_level(&rover_installer)?;
        // Read before installing, so that a lockfile this can't update
        // stops the install rather than leaving a plugin it doesn't record.
        if let Some(level) = &level {
            PluginLockfile::load(&level.join(LOCKFILE))?;
        }

        let installed = PluginInstaller::new(client_config, rover_installer, self.force)
            .requested_by(Some(RequestOrigin::PluginArgument))
            .without_downloads(self.download_control())
            .install(&self.plugin)
            .await?;

        // With downloads disabled, a floating request was resolved against
        // nothing: the newest release on disk is not what it resolves to, and
        // locking it would say so. An exact request needs no resolving.
        let resolved = self.download_control().is_none() || !self.plugin.request().is_floating();
        if let Some(level) =
            level.filter(|level| resolved && belongs_in_lockfile(&installed, level))
        {
            let path = level.join(LOCKFILE);
            // Read again rather than reuse the first read: another install
            // may have recorded a plugin while this one downloaded.
            PluginLockfile::load(&path)?
                .unwrap_or_default()
                .with(LockedPlugin {
                    name: self.plugin.name(),
                    requested: self.plugin.request(),
                    resolved: installed.version.clone(),
                    checksum: None,
                })
                .write(&path, &LockfileWrite(()))?;
        }

        Ok(RoverOutput::CliOutput(Box::new(PluginInstallOutput {
            plugins: vec![installed],
        })))
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
/// That is the global level, unless `APOLLO_NODE_MODULES_BIN_DIR` sends
/// plugins into an npm package's `node_modules/.bin`. An install there needs no
/// home directory, so none is looked for, and no lockfile is read.
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

#[cfg(test)]
mod tests {
    use camino::Utf8Path;
    use rstest::rstest;
    use semver::Version;
    use speculoos::prelude::*;

    use super::*;
    use crate::command::install::PluginLevel;

    const IN_BIN: &str = "/home/me/.rover/bin/supergraph-v2.9.3";

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

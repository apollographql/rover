mod install;
mod list;

use anyhow::anyhow;
use camino::{Utf8Path, Utf8PathBuf};
use clap::Parser;
use serde::Serialize;

pub(crate) use self::install::PluginInstall;
use self::list::PluginList;
use crate::{
    RoverError, RoverOutput, RoverResult,
    plugin::{
        discovery::{ManifestDirs, project_in_scope},
        manifest::MANIFEST_FILE,
    },
    utils::client::StudioClientConfig,
};

/// Permission to write a plugin lockfile, which only the `rover plugin` verbs
/// that install or remove plugins can grant: a lockfile changes on an
/// explicit install or removal and at no other time.
///
/// Its field is private to this module, so nothing outside it can make one.
/// Commands that install plugins on the fly, such as `rover supergraph
/// compose`, share the installer but not this module, and so cannot write a
/// lockfile by accident.
pub struct LockfileWrite(());

#[cfg(test)]
impl LockfileWrite {
    /// A permit for tests of the lockfile itself, outside this module.
    pub(crate) const fn for_tests() -> Self {
        Self(())
    }
}

#[derive(Debug, Serialize, Parser)]
pub struct Plugins {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Debug, Serialize, Parser)]
pub enum Command {
    /// Download and install an officially supported plugin
    Install(PluginInstall),

    /// List the plugins installed in this project and globally
    List(PluginList),
}

impl Plugins {
    pub async fn run(
        &self,
        override_install_path: Option<Utf8PathBuf>,
        client_config: StudioClientConfig,
    ) -> RoverResult<RoverOutput> {
        match &self.command {
            Command::Install(command) => command.run(override_install_path, client_config).await,
            Command::List(command) => command.run(override_install_path),
        }
    }
}

impl Plugins {
    /// The project manifest this invocation names with `--manifest-path`, made
    /// absolute, if any - the one manifest every section of it is read from
    /// (FR78 of the profile-configuration spec), not only the plugin
    /// declarations.
    pub(crate) fn named_manifest(&self) -> Option<RoverResult<Utf8PathBuf>> {
        match &self.command {
            Command::Install(command) => command.named_manifest(),
            Command::List(command) => command.named_manifest(),
        }
    }
}

/// The manifest of the project a verb acts on, or `None` for the global
/// level alone: the `--manifest-path` manifest when one is `named` (FR25),
/// none when the global level is asked for (FR24), and otherwise the manifest
/// of the project in scope, if there is one (FR23). `conflict` is the error
/// for a named manifest when `APOLLO_ROVER_GLOBAL` asks for the global level;
/// clap refuses the two flags together, so only the variable gets that far.
fn target_manifest(
    named: Option<&Utf8Path>,
    global: bool,
    apollo_home: Option<&Utf8Path>,
    conflict: impl FnOnce() -> RoverError,
) -> RoverResult<Option<Utf8PathBuf>> {
    match (named, global) {
        (Some(_), true) => Err(conflict()),
        (Some(manifest), false) => absolute_manifest(manifest).map(Some),
        (None, true) => Ok(None),
        (None, false) => Ok(
            project_in_scope(ManifestDirs::for_this_process(apollo_home))
                .map(|project| project.join(MANIFEST_FILE)),
        ),
    }
}

/// `manifest`, made absolute against the working directory. The one place
/// that happens, so a verb and settings resolution (FR78 of the
/// profile-configuration spec) always mean the same file.
fn absolute_manifest(manifest: &Utf8Path) -> RoverResult<Utf8PathBuf> {
    let absolute = std::path::absolute(manifest)
        .map_err(|err| anyhow!(err).context(format!("Couldn't find `{manifest}`.")))?;
    Ok(Utf8PathBuf::try_from(absolute).map_err(|err| anyhow!(err))?)
}

#[cfg(test)]
impl Plugins {
    /// The `install` verb this parsed to, for tests comparing the two spellings.
    pub(crate) fn install(&self) -> Option<&PluginInstall> {
        match &self.command {
            Command::Install(command) => Some(command),
            Command::List(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use camino::Utf8Path;
    use rstest::rstest;
    use speculoos::prelude::*;

    use super::*;

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
}

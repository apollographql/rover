mod install;

use camino::Utf8PathBuf;
use clap::Parser;
use serde::Serialize;

pub(crate) use self::install::PluginInstall;
use crate::{RoverOutput, RoverResult, utils::client::StudioClientConfig};

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
}

impl Plugins {
    pub async fn run(
        &self,
        override_install_path: Option<Utf8PathBuf>,
        client_config: StudioClientConfig,
    ) -> RoverResult<RoverOutput> {
        match &self.command {
            Command::Install(command) => command.run(override_install_path, client_config).await,
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
        }
    }
}

#[cfg(test)]
impl Plugins {
    /// The `install` verb this parsed to, for tests comparing the two spellings.
    pub(crate) fn install(&self) -> Option<&PluginInstall> {
        match &self.command {
            Command::Install(command) => Some(command),
        }
    }
}

mod install;

use camino::Utf8PathBuf;
use clap::Parser;
use serde::Serialize;

pub(crate) use self::install::PluginInstall;
use crate::{RoverOutput, RoverResult, utils::client::StudioClientConfig};

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

#[cfg(test)]
impl Plugins {
    /// The `install` verb this parsed to, for tests comparing the two spellings.
    pub(crate) fn install(&self) -> Option<&PluginInstall> {
        match &self.command {
            Command::Install(command) => Some(command),
        }
    }
}

pub(crate) mod compose;
mod fetch;

#[cfg(feature = "composition-js")]
mod resolve_config;
#[cfg(feature = "composition-js")]
pub(crate) use resolve_config::resolve_supergraph_yaml;

use camino::Utf8PathBuf;
use clap::Parser;
use serde::Serialize;

use crate::utils::client::StudioClientConfig;
use crate::{RoverOutput, RoverResult};

#[derive(Debug, Serialize, Parser)]
pub struct Supergraph {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Debug, Serialize, Parser)]
pub enum Command {
    /// Locally compose supergraph SDL from a set of subgraph schemas
    Compose(compose::Compose),

    /// Fetch supergraph SDL from the graph registry
    Fetch(fetch::Fetch),
}

impl Supergraph {
    pub fn run(
        &self,
        override_install_path: Option<Utf8PathBuf>,
        client_config: StudioClientConfig,
    ) -> RoverResult<RoverOutput> {
        match &self.command {
            Command::Fetch(command) => command.run(client_config),
            Command::Compose(command) => command.run(override_install_path, client_config),
        }
    }
}

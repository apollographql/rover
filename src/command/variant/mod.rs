mod list;

use serde::Serialize;
use structopt::StructOpt;

use crate::command::RoverStdout;
use crate::utils::client::StudioClientConfig;
use crate::Result;

#[derive(Debug, Serialize, StructOpt)]
pub struct Variant {
    #[structopt(subcommand)]
    command: Command,
}

#[derive(Debug, Serialize, StructOpt)]
pub enum Command {
    /// Lists the available variants for a graph ref
    List(list::List),
}

impl Variant {
    pub fn run(&self, client_config: StudioClientConfig) -> Result<RoverStdout> {
        match &self.command {
            Command::List(command) => command.run(client_config),
        }
    }
}

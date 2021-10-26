use structopt::StructOpt;

use crate::command::Command;

#[derive(Debug, StructOpt)]
#[structopt(
    name = "rover-fed",
    about = "A utility for composing multiple subgraphs into a supergraph"
)]
pub struct RoverFed {
    #[structopt(subcommand)]
    command: Command,

    /// Print output as JSON.
    #[structopt(long, global = true)]
    json: bool,
}

impl RoverFed {
    pub fn run(&self) -> Result<(), anyhow::Error> {
        match &self.command {
            Command::Compose(command) => command.run(self.json),
        }
    }
}

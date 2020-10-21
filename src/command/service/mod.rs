mod push;

use anyhow::Result;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
pub struct Service {
    #[structopt(subcommand)]
    command: Command,
}

#[derive(Debug, StructOpt)]
pub enum Command {
    /// Push a schema from a file
    Push(push::Push),
}

impl Service {
    pub fn run(&self) -> Result<()> {
        match &self.command {
            Command::Push(service) => service.run(),
        }
    }
}

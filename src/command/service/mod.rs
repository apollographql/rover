mod push;

use anyhow::Result;
use serde::Serialize;
use structopt::StructOpt;

#[derive(Debug, Serialize, StructOpt)]
pub struct Service {
    #[structopt(subcommand)]
    command: Command,
}

#[derive(Debug, Serialize, StructOpt)]
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

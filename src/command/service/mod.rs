mod delete;
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
    /// Delete a service from a federated graph
    Delete(delete::Delete),
}

impl Service {
    pub fn run(&self) -> Result<()> {
        match &self.command {
            Command::Push(service) => service.run(),
            Command::Delete(service) => service.run(),
        }
    }
}

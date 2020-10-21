mod fetch;
mod push;

use anyhow::Result;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
pub struct Schema {
    #[structopt(subcommand)]
    command: Command,
}

#[derive(Debug, StructOpt)]
pub enum Command {
    /// 🐶 Get a schema given an identifier
    Fetch(fetch::Fetch),
    /// Push a schema from a file
    Push(push::Push),
}

impl Schema {
    pub fn run(&self) -> Result<()> {
        match &self.command {
            Command::Fetch(fetch) => fetch.run(),
            Command::Push(schema) => schema.run(),
        }
    }
}

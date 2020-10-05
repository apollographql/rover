mod fetch;
mod stash;

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
    /// Stash a schema from a file or introspection endpoint
    Stash(stash::Stash),
}

impl Schema {
    pub fn run(&self) -> Result<()> {
        match &self.command {
            Command::Fetch(fetch) => fetch.run(),
            Command::Stash(schema) => schema.run(),
        }
    }
}

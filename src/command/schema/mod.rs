mod fetch;

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
}

impl Schema {
    pub fn run(&self) -> Result<()> {
        match &self.command {
            Command::Fetch(f) => f.run(),
        }
    }
}

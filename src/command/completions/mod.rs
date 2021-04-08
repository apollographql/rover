mod generate;

use serde::Serialize;
use structopt::{clap::App, StructOpt};

use crate::{command::RoverStdout, Result};

#[derive(Debug, Serialize, StructOpt)]
pub struct Completions {
    #[structopt(subcommand)]
    command: Command,
}

#[derive(Debug, Serialize, StructOpt)]
pub enum Command {
    /// Generate shell completions for Rover
    Generate(generate::Generate),
}

impl Completions {
    pub fn run(&self, app: App) -> Result<RoverStdout> {
        match &self.command {
            Command::Generate(command) => command.run(app),
        }
    }
}

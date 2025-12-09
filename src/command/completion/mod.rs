mod bash;
mod zsh;

use clap::Parser;
use serde::Serialize;

use crate::{RoverOutput, RoverResult};

#[derive(Debug, Serialize, Parser)]
pub struct Completion {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Debug, Serialize, Parser)]
enum Command {
    /// Generate bash completion script
    Bash(bash::Bash),

    /// Generate zsh completion script
    Zsh(zsh::Zsh),
}

impl Completion {
    pub fn run(&self) -> RoverResult<RoverOutput> {
        match &self.command {
            Command::Bash(command) => command.run(),
            Command::Zsh(command) => command.run(),
        }
    }
}

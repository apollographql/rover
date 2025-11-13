use clap::{CommandFactory, Parser};
use clap_complete::{generate, shells::Zsh as ZshShell};
use serde::Serialize;

use crate::{RoverOutput, RoverResult, cli::Rover};

#[derive(Debug, Serialize, Parser)]
pub struct Zsh {}

impl Zsh {
    pub fn run(&self) -> RoverResult<RoverOutput> {
        let mut cmd = Rover::command();
        let name = "rover".to_string();
        generate(ZshShell, &mut cmd, &name, &mut std::io::stdout());
        Ok(RoverOutput::EmptySuccess)
    }
}

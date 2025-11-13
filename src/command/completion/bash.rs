use clap::{CommandFactory, Parser};
use clap_complete::{generate, shells::Bash as BashShell};
use serde::Serialize;

use crate::{RoverOutput, RoverResult, cli::Rover};

#[derive(Debug, Serialize, Parser)]
pub struct Bash {}

impl Bash {
    pub fn run(&self) -> RoverResult<RoverOutput> {
        let mut cmd = Rover::command();
        let name = "rover".to_string();
        generate(BashShell, &mut cmd, &name, &mut std::io::stdout());
        Ok(RoverOutput::EmptySuccess)
    }
}

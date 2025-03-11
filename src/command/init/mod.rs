use clap::Parser;
use serde::Serialize;

use crate::{RoverOutput, RoverResult};

#[derive(Debug, Serialize, Parser)]
pub struct Init {
    /// Overwrite any existing binary without prompting for confirmation.
    #[arg(long = "force", short = 'f')]
    pub(crate) force: bool,
}

impl Init {
    pub async fn run(&self) -> RoverResult<RoverOutput> {
        eprintln!("\nWelcome! This command helps you initialize a new GraphQL API project using Apollo Federation with Apollo Router.\n");

        Ok(RoverOutput::EmptySuccess)
    }
}

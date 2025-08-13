use clap::Parser;
use serde::Serialize;

use crate::RoverOutput::EmptySuccess;
use crate::{RoverOutput, RoverResult};

#[derive(Debug, Serialize, Parser)]
pub struct Connector {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Debug, Parser, Clone, Serialize)]
#[clap(about = "Work with Apollo Connectors")]
pub enum Command {
    /// Generate a new connector
    Generate,
    /// Run a single connector
    Run,
    /// Run tests for one or more connectors
    Test,
    /// List all available connectors
    List,
}

impl Connector {
    pub(crate) async fn run(&self) -> RoverResult<RoverOutput> {
        use Command::*;
        match self.command {
            Generate => {
                // TODO: Logic for generating a new connector
                Ok(EmptySuccess)
            }
            Run => {
                // TODO: Logic for running a single connector
                Ok(EmptySuccess)
            }
            Test => {
                // TODO: Logic for running tests for connectors
                Ok(EmptySuccess)
            }
            List => {
                // TODO: Logic for listing all available connectors
                Ok(EmptySuccess)
            }
        }
    }
}

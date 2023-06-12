mod merge;

pub use merge::Merge;

use clap::Parser;
use serde::Serialize;

use crate::{RoverOutput, RoverResult};

#[derive(Debug, Clone, Parser, Serialize)]
pub struct Tools {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Clone, Debug, Parser, Serialize)]
enum Command {
    /// Merge multiple schema files into one
    SchemaMerge(Merge),
}

impl Tools {
    pub(crate) fn run(&self) -> RoverResult<RoverOutput> {
        match &self.command {
            Command::SchemaMerge(merge) => merge.run(),
        }
    }
}

mod manifest;
pub(super) mod printer;

// Full Generate command (file discovery, glob args, output) coming in a later PR.

use clap::Parser;
use serde::Serialize;

use crate::{RoverOutput, RoverResult};

#[derive(Debug, Serialize, Parser)]
pub struct Generate {}

impl Generate {
    pub async fn run(&self) -> RoverResult<RoverOutput> {
        todo!("manifest generation coming in a later PR")
    }
}

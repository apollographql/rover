use saucer::{clap, Parser};
use serde::Serialize;

use crate::Result;

use crate::command::RoverOutput;

#[derive(Serialize, Debug, Parser)]
/// List all configuration profiles
pub struct Template {}

impl Template {
  pub fn run(&self) -> Result<RoverOutput> {
    return Ok(RoverOutput::EmptySuccess);
  }
}

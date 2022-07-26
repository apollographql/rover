use crate::{command::RoverOutput, Result};

use super::shortlinks;

use saucer::{clap, Parser};
use serde::Serialize;

#[derive(Debug, Serialize, Parser)]
pub struct List {}

impl List {
    pub fn run(&self) -> Result<RoverOutput> {
        Ok(RoverOutput::DocsList(
            shortlinks::get_shortlinks_with_description(),
        ))
    }
}

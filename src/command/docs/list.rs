use crate::{command::RoverOutput, Result};

use super::shortlinks;

use serde::Serialize;
use structopt::StructOpt;

#[derive(Debug, Serialize, StructOpt)]
pub struct List {}

impl List {
    pub fn run(&self) -> Result<RoverOutput> {
        Ok(RoverOutput::DocsList(
            shortlinks::get_shortlinks_with_description(),
        ))
    }
}

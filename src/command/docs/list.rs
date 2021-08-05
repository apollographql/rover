use crate::{command::RoverStdout, Result};

use super::shortlinks;

use serde::Serialize;
use structopt::StructOpt;

#[derive(Debug, Serialize, StructOpt)]
pub struct List {}

impl List {
    pub fn run(&self) -> Result<RoverStdout> {
        Ok(RoverStdout::DocsList(
            shortlinks::get_shortlinks_with_description(),
        ))
    }
}

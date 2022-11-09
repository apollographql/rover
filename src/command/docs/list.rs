use crate::{RoverOutput, RoverResult};

use super::shortlinks;

use clap::Parser;
use serde::Serialize;

#[derive(Debug, Serialize, Parser)]
pub struct List {}

impl List {
    pub fn run(&self) -> RoverResult<RoverOutput> {
        Ok(RoverOutput::DocsList(
            shortlinks::get_shortlinks_with_description(),
        ))
    }
}

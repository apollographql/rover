use clap::Parser;

use super::shortlinks;
use crate::{RoverOutput, RoverResult};

#[derive(Debug, Parser)]
pub struct List {}

impl List {
    pub fn run(&self) -> RoverResult<RoverOutput> {
        Ok(RoverOutput::DocsList(shortlinks::get_shortlinks_with_info()))
    }
}

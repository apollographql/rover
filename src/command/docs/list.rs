use clap::Parser;
use serde::Serialize;

use super::shortlinks;
use crate::{RoverOutput, RoverResult};

#[derive(Debug, Serialize, Parser)]
pub struct List {}

impl List {
    pub fn run(&self) -> RoverResult<RoverOutput> {
        Ok(RoverOutput::DocsList(shortlinks::get_shortlinks_with_info()))
    }
}

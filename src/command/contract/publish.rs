use serde::Serialize;
use clap::Parser;

use crate::{RoverResult, RoverOutput};

#[derive(Debug, Serialize, Parser)]
pub struct Publish { }

impl Publish {
    pub fn run(&self) -> RoverResult<RoverOutput> {
        eprintln!("Hello, world!");
        Ok(RoverOutput::EmptySuccess)
    }
}
use crate::command::RoverOutput;
use crate::error::metadata::code::Code;
use crate::Result;
use clap::Parser;
use serde::Serialize;

#[derive(Debug, Serialize, Parser)]
pub struct Explain {
    /// The full error code. For example, E020
    #[clap(name = "CODE")]
    code: Code,
}

impl Explain {
    pub fn run(&self) -> Result<RoverOutput> {
        let explanation = &self.code.explain();
        Ok(RoverOutput::ErrorExplanation(explanation.clone()))
    }
}

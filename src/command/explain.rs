use crate::{RoverErrorCode, RoverOutput, RoverResult};

use clap::Parser;
use serde::Serialize;

#[derive(Debug, Serialize, Parser)]
pub struct Explain {
    /// The full error code. For example, E020
    #[arg(value_name = "CODE")]
    code: RoverErrorCode,
}

impl Explain {
    pub fn run(&self) -> RoverResult<RoverOutput> {
        let explanation = &self.code.explain();
        Ok(RoverOutput::ErrorExplanation(explanation.clone()))
    }
}

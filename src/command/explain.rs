use crate::command::RoverStdout;
use crate::error::metadata::code::Code;
use crate::Result;
use serde::Serialize;
use structopt::StructOpt;

#[derive(Debug, Serialize, StructOpt)]
pub struct Explain {
    /// The full error code. For example, E020
    #[structopt(name = "CODE")]
    code: Code,
}

impl Explain {
    pub fn run(&self) -> Result<RoverStdout> {
        let explanation = &self.code.explain();
        Ok(RoverStdout::Markdown(explanation.clone()))
    }
}

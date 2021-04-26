use crate::command::RoverStdout;
use crate::Result;
use crate::error::metadata::code::Code;
use structopt::StructOpt;
use serde::Serialize;

#[derive(Debug, Serialize, StructOpt)]
pub struct Explain {
    /// The full error code. For example, E020
    #[structopt(name = "CODE")]
    code: Code,
}

impl Explain {
    pub fn run(&self) -> Result<RoverStdout> {
        let explanation = &self.code.explain();

        // if we're printing all codes, we don't need to pretty print them
        if self.code == Code::EALL {
            Ok(RoverStdout::PlainText(explanation.clone()))
        } else {
            Ok(RoverStdout::Markdown(explanation.clone()))
        }
    }
}

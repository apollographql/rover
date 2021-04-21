use crate::command::RoverStdout;
use crate::Result;
use crate::error::metadata::code::Code;
use structopt::StructOpt;
use serde::Serialize;
use std::convert::TryFrom;
use ansi_term::Colour::{Red};

#[derive(Debug, Serialize, StructOpt)]
pub struct Explain {
    /// The full error code. For example, E020
    #[structopt(name = "CODE")]
    code: String,

    #[structopt(long="all", hidden=true)]
    all: bool
}

impl Explain {
    pub fn run(&self) -> Result<RoverStdout> {
        // print out all error explanations in their MD format
        if self.all {
            println!("{}", Code::explain_all());
            return Ok(RoverStdout::None)
        }

        let code = Code::try_from(self.code.as_str())?;
        let explanation = code.explain();

        let label = Red.bold().paint(&self.code);
        eprintln!("{}\n\n{}", label, explanation);
        
        Ok(RoverStdout::None)
    }
}

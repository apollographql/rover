use std::str;

use serde::Serialize;
use structopt::{
    clap::{App, Shell},
    StructOpt,
};

use crate::PKG_NAME;
use crate::{command::RoverStdout, Result};

#[derive(Debug, Serialize, StructOpt)]
pub struct Completions {
    #[structopt(case_insensitive = true, possible_values = &Shell::variants())]
    #[serde(skip_serializing)]
    /// The shell to generate completions for.
    pub shell: Shell,
}

impl Completions {
    pub fn run(&self, mut app: App) -> Result<RoverStdout> {
        let mut mystr = Vec::new();
        app.gen_completions_to(PKG_NAME, self.shell, &mut mystr);
        let completion = str::from_utf8(&mystr)?;
        Ok(RoverStdout::ShellCompletion(completion.to_string()))
    }
}

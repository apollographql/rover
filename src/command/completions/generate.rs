use std::str;

use crate::PKG_NAME;
use crate::{command::RoverStdout, Result};

use serde::Serialize;
use structopt::clap::{App, Shell};
use structopt::StructOpt;

#[derive(Debug, Serialize, StructOpt)]
pub struct Generate {
    #[structopt(case_insensitive = true, possible_values = &Shell::variants())]
    #[serde(skip_serializing)]
    pub shell: Shell,
}

impl Generate {
    pub fn run(&self, mut app: App) -> Result<RoverStdout> {
        let mut mystr = Vec::new();
        app.gen_completions_to(PKG_NAME, self.shell, &mut mystr);
        let completion = str::from_utf8(&mystr)?;
        Ok(RoverStdout::ShellCompletion(completion.to_string()))
    }
}

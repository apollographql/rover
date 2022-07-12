use anyhow::Result;
use clap::Parser;

use crate::tools::{CargoRunner, NpmRunner};

#[derive(Debug, Parser)]
pub struct Lint {}

impl Lint {
    pub fn run(&self, verbose: bool) -> Result<()> {
        let cargo_runner = CargoRunner::new(verbose)?;
        cargo_runner.lint()?;
        let npm_runner = NpmRunner::new(verbose)?;
        npm_runner.lint()?;
        Ok(())
    }
}

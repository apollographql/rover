use anyhow::Result;
use clap::Parser;

use crate::tools::{CargoRunner, NpmRunner};

#[derive(Debug, Parser)]
pub struct Lint {}

impl Lint {
    pub fn run(&self) -> Result<()> {
        CargoRunner::new()?.lint()?;
        // TODO: do we actually need to do npm stuff?
        NpmRunner::new()?.lint()
    }
}

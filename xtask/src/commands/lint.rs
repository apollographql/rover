use anyhow::Result;
use clap::Parser;

#[cfg(not(windows))]
use crate::tools::LycheeRunner;

use crate::tools::{CargoRunner, NpmRunner};

#[derive(Debug, Parser)]
pub struct Lint {}

impl Lint {
    pub fn run(&self) -> Result<()> {
        CargoRunner::new()?.lint()?;
        NpmRunner::new()?.lint()?;
        lint_links()
    }
}

#[cfg(not(windows))]
fn lint_links() -> Result<()> {
    LycheeRunner::new()?.lint()
}

#[cfg(windows)]
fn lint_links() -> Result<()> {
    eprintln!("Skipping the lint checker.");

    Ok(())
}

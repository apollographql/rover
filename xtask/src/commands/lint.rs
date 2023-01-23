use anyhow::Result;
use clap::Parser;

#[cfg(not(windows))]
use crate::tools::LycheeRunner;

use crate::tools::{CargoRunner, NpmRunner};

#[derive(Debug, Parser)]
pub struct Lint {}

impl Lint {
    pub fn run(&self) -> Result<()> {
        let cargo_runner = CargoRunner::new()?;
        cargo_runner.lint()?;
        let npm_runner = NpmRunner::new()?;
        npm_runner.lint()?;
        lint_links()?;

        Ok(())
    }
}

#[cfg(not(windows))]
fn lint_links() -> Result<()> {
    let lychee_runner = LycheeRunner::new()?;
    lychee_runner.lint()?;

    Ok(())
}

#[cfg(windows)]
fn lint_links(_verbose: bool) -> Result<()> {
    println!("Skipping the lint checcker.");

    Ok(())
}

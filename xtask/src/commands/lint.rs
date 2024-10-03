#[cfg(not(windows))]
use crate::tools::LycheeRunner;
use anyhow::Result;
use clap::Parser;

use crate::tools::{CargoRunner, GitRunner, NpmRunner};

#[derive(Debug, Parser)]
pub struct Lint {
    /// The current (most recent SHA) to use for comparison
    #[arg(long = "branch-name", default_value = "main")]
    pub(crate) branch_name: String,

    #[arg(long, short, action)]
    pub(crate) force: bool,
}

impl Lint {
    pub async fn run(&self) -> Result<()> {
        CargoRunner::new()?.lint()?;
        NpmRunner::new()?.lint()?;
        lint_links(&self.branch_name, self.force).await
    }
}

#[cfg(not(windows))]
async fn lint_links(branch_name: &str, force: bool) -> Result<()> {
    if force
        || GitRunner::tmp()?
            .get_changed_files(branch_name)?
            .iter()
            .any(|path| path.extension().unwrap_or_default() == "md")
    {
        LycheeRunner::new()?.lint().await
    } else {
        eprintln!("Skipping the lint checker for '.md' files as no '.md' files have changed.");
        Ok(())
    }
}

#[cfg(windows)]
async fn lint_links(branch_name: &str, force: bool) -> Result<()> {
    eprintln!("Skipping the lint checker.");

    Ok(())
}

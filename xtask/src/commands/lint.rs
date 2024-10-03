use anyhow::Result;
use clap::Parser;

#[cfg(not(windows))]
use crate::tools::LycheeRunner;

use crate::tools::{CargoRunner, GitRunner, NpmRunner};

#[derive(Debug, Parser)]
pub struct Lint {
    /// The current (most recent SHA) to use for comparison
    #[arg(long = "branch-name")]
    pub(crate) branch_name: String,
}

impl Lint {
    pub async fn run(&self) -> Result<()> {
        CargoRunner::new()?.lint()?;
        NpmRunner::new()?.lint()?;
        lint_links(&self.branch_name).await
    }
}

#[cfg(not(windows))]
async fn lint_links(branch_name: &str) -> Result<()> {
    let changed_files = GitRunner::tmp()?.get_changed_files(branch_name)?;
    if changed_files
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
async fn lint_links(branch_name: &str) -> Result<()> {
    eprintln!("Skipping the lint checker.");

    Ok(())
}

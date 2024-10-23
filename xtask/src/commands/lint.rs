#[cfg(not(windows))]
use crate::tools::LycheeRunner;
use anyhow::Result;
use clap::Parser;

use crate::tools::{CargoRunner, GitRunner, NpmRunner};

#[derive(Debug, Parser)]
pub struct Lint {
    /// The current (most recent SHA) to use for comparison
    #[arg(long = "current-sha", default_value = "main")]
    pub(crate) current_sha: String,

    #[arg(long, short, action)]
    pub(crate) force: bool,
}

impl Lint {
    pub async fn run(&self) -> Result<()> {
        println!("A");
        CargoRunner::new()?.lint()?;
        println!("B");
        NpmRunner::new()?.lint()?;
        println!("C");
        lint_links(&self.current_sha, self.force).await
    }
}

#[cfg(not(windows))]
async fn lint_links(current_sha: &str, force: bool) -> Result<()> {
    if force
        || GitRunner::tmp()?
            .get_changed_files(current_sha)?
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

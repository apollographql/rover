use anyhow::Result;
use clap::Parser;

#[cfg(not(windows))]
use crate::tools::{GitRunner, LycheeRunner};

use crate::tools::{CargoRunner, NpmRunner};

#[derive(Debug, Parser)]
pub struct Lint {
    #[arg(long, short, action)]
    pub(crate) force: bool,
}

impl Lint {
    pub async fn run(&self) -> Result<()> {
        CargoRunner::new()?.lint()?;
        NpmRunner::new()?.lint()?;
        lint_links(self.force).await
    }
}

#[cfg(not(windows))]
async fn lint_links(force: bool) -> Result<()> {
    if force
        || GitRunner::get_changed_files()?
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
async fn lint_links(force: bool) -> Result<()> {
    eprintln!("Skipping the lint checker.");

    Ok(())
}

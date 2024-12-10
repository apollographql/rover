use anyhow::Result;
use clap::Parser;

#[cfg(not(windows))]
use crate::tools::LycheeRunner;
use crate::tools::{CargoRunner, GitRunner, GithubRepo, NpmRunner};

#[derive(Debug, Parser)]
pub struct Lint {
    /// The current (most recent SHA) to use for comparison
    #[arg(long = "current-sha", default_value = "main")]
    pub(crate) current_sha: String,

    #[arg(long)]
    pub(crate) git_remote: Option<GithubRepo>,

    #[arg(long, short, action)]
    pub(crate) force: bool,
}

impl Lint {
    pub async fn run(&self) -> Result<()> {
        CargoRunner::new()?.lint()?;
        NpmRunner::new()?.lint()?;
        lint_links(
            self.git_remote.clone().unwrap_or_default(),
            &self.current_sha,
            self.force,
        )
        .await
    }
}

#[cfg(not(windows))]
async fn lint_links(remote: GithubRepo, current_sha: &str, force: bool) -> Result<()> {
    if force
        || GitRunner::tmp(remote)?
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

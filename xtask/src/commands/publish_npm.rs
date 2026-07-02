use anyhow::{Context, Result};
use camino::Utf8PathBuf;
use clap::Parser;

use crate::{
    tools::NpmRunner,
    utils::{PKG_PROJECT_ROOT, PKG_VERSION},
};

#[derive(Debug, Parser)]
pub struct PublishNpm {
    /// Directory of the npm package to publish.
    /// Defaults to installers/npm/@apollo/rover.
    #[arg(long)]
    dir: Option<Utf8PathBuf>,

    /// Simulate without actually publishing
    #[arg(long)]
    dry_run: bool,

    /// Override the npm dist-tag (auto-detected from PKG_VERSION if omitted)
    #[arg(long)]
    tag: Option<String>,
}

impl PublishNpm {
    pub fn run(&self) -> Result<()> {
        let runner = NpmRunner::new()?;
        let dir = self.dir.clone().unwrap_or_else(|| {
            PKG_PROJECT_ROOT
                .join("installers")
                .join("npm")
                .join("@apollo")
                .join("rover")
        });
        let tag = self
            .tag
            .clone()
            .or_else(|| resolve_tag_from_version().ok().flatten());
        if self.dir.is_none() {
            // No --dir means we're publishing the main @apollo/rover package for real, after
            // its platform packages have already been published — generate in full (non-stub)
            // mode so it ships with real optionalDependencies and a populated PLATFORMS map.
            runner
                .prepare_package(false)
                .with_context(|| "Could not prepare npm package.")?;
        }
        runner
            .publish(&dir, self.dry_run, tag.as_deref())
            .with_context(|| format!("Failed to publish npm package at {dir}"))
    }
}

fn resolve_tag_from_version() -> Result<Option<String>> {
    let v = semver::Version::parse(&PKG_VERSION).with_context(|| {
        format!(
            "Could not parse Rover version '{}' as semver.",
            *PKG_VERSION
        )
    })?;
    if !v.pre.is_empty() {
        Ok(Some("beta".to_string()))
    } else {
        Ok(None)
    }
}

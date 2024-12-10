use std::collections::BTreeMap;

use anyhow::Result;
use clap::Parser;
use rover_std::Fs;

use crate::tools::{GitRunner, GithubRepo, NpmRunner};

use camino::Utf8PathBuf;

#[derive(Debug, Parser)]
pub struct Docs {
    #[arg(long, short, default_value = "./dev-docs")]
    path: Utf8PathBuf,

    // The monodocs branch to check out
    #[arg(long, short, default_value = "main")]
    pub(crate) branch: String,

    // The monodocs org to clone
    #[arg(long, short, default_value = "apollographql")]
    pub(crate) org: String,
}

impl Docs {
    pub fn run(&self) -> Result<()> {
        let git_runner = GitRunner::new(
            GithubRepo::builder()
                .org(self.org.clone())
                .name("docs".to_string())
                .build(),
            &self.path,
        )?;
        let docs = git_runner.clone(&self.branch)?;
        let local_sources_yaml_path = docs.join("sources").join("local.yml");
        let local_sources_yaml = Fs::read_file(&local_sources_yaml_path)?;
        let mut local_sources: BTreeMap<String, Utf8PathBuf> =
            serde_yaml::from_str(&local_sources_yaml)?;
        local_sources.insert(
            "rover".to_string(),
            crate::utils::PKG_PROJECT_ROOT.join("docs").join("source"),
        );
        Fs::write_file(
            &local_sources_yaml_path,
            serde_yaml::to_string(&local_sources)?,
        )?;
        let npm_runner = NpmRunner::new()?;
        npm_runner.dev_docs(&self.path)?;
        Ok(())
    }
}

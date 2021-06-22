use crate::tools::Runner;

use std::convert::TryFrom;

use anyhow::{Context, Result};
use assert_fs::TempDir;
use camino::Utf8PathBuf;

pub(crate) struct GitRunner {
    temp_dir_path: Utf8PathBuf,
    runner: Runner,

    // we store _temp_dir here since its Drop implementation deletes the directory
    _temp_dir: TempDir,
}

impl GitRunner {
    pub(crate) fn new(verbose: bool) -> Result<Self> {
        let runner = Runner::new("git", verbose)?;
        let temp_dir = TempDir::new().with_context(|| "Could not create temp directory")?;
        let temp_dir_path = Utf8PathBuf::try_from(temp_dir.path().to_path_buf())
            .with_context(|| "Temp directory was not valid Utf-8")?;

        Ok(GitRunner {
            runner,
            temp_dir_path,
            _temp_dir: temp_dir,
        })
    }

    pub(crate) fn clone_supergraph_demo(&self) -> Result<Utf8PathBuf> {
        let repo_name = "supergraph-demo";
        let repo_url = format!("https://github.com/apollographql/{}", repo_name);
        self.runner
            .exec(&["clone", &repo_url], &self.temp_dir_path, None)?;

        let repo_path = self.temp_dir_path.join(repo_name);
        Ok(repo_path)
    }
}

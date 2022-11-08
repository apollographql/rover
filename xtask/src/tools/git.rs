use std::{convert::TryFrom, fs};

use crate::tools::Runner;

use anyhow::{Context, Result};
use assert_fs::TempDir;
use camino::Utf8PathBuf;

pub(crate) struct GitRunner {
    runner: Runner,

    // we store _temp_dir here since its Drop implementation deletes the directory
    repo: RepoLocation,
}

enum RepoLocation {
    Local(LocalRepo),
    Tmp(TmpRepo),
}

struct LocalRepo {
    path: Utf8PathBuf,
}

struct TmpRepo {
    path: Utf8PathBuf,
    _handle: TempDir,
}

impl GitRunner {
    pub(crate) fn new(verbose: bool, path: &Utf8PathBuf) -> Result<Self> {
        let runner = Runner::new("git", verbose)?;
        Ok(GitRunner {
            runner,
            repo: RepoLocation::Local(LocalRepo { path: path.clone() }),
        })
    }
    pub(crate) fn tmp(verbose: bool) -> Result<Self> {
        let runner = Runner::new("git", verbose)?;
        let temp_dir = TempDir::new().with_context(|| "Could not create temp directory")?;
        let temp_dir_path = Utf8PathBuf::try_from(temp_dir.path().to_path_buf())
            .with_context(|| "Temp directory was not valid Utf-8")?;

        Ok(GitRunner {
            runner,
            repo: RepoLocation::Tmp(TmpRepo {
                path: temp_dir_path,
                _handle: temp_dir,
            }),
        })
    }

    fn get_path(&self) -> Result<Utf8PathBuf> {
        let path = match &self.repo {
            RepoLocation::Local(local) => local.path.clone(),
            RepoLocation::Tmp(tmp) => tmp.path.clone(),
        };

        if fs::metadata(&path).is_err() {
            fs::create_dir_all(&path)?;
        }
        Ok(path)
    }

    fn clone(&self, org: &str, name: &str, branch: &str) -> Result<Utf8PathBuf> {
        let url = format!("https://github.com/{}/{}", org, name);
        let path = self.get_path()?;
        if let RepoLocation::Local(local) = &self.repo {
            if fs::metadata(&local.path.join(".git")).is_ok() {
                self.runner
                    .exec(&["reset", "--hard", "HEAD"], &path, None)?;
                self.runner.exec(&["checkout", branch], &path, None)?;
                self.runner.exec(&["pull"], &path, None)?;
                return Ok(local.path.clone());
            }
        }

        self.runner.exec(
            &["clone", &url, "--branch", branch, path.as_ref()],
            &crate::utils::PKG_PROJECT_ROOT,
            None,
        )?;

        Ok(path)
    }

    pub(crate) fn clone_supergraph_demo(&self, org: &str, branch: &str) -> Result<Utf8PathBuf> {
        self.clone(org, "supergraph-demo", branch)
    }

    pub(crate) fn clone_docs(&self, org: &str, branch: &str) -> Result<Utf8PathBuf> {
        self.clone(org, "docs", branch)
    }

    pub(crate) fn checkout_rover_version(&self, rover_version: &str) -> Result<Utf8PathBuf> {
        let repo_path = self.clone("apollographql", "rover", "main")?;

        self.runner.exec(
            &["checkout", &format!("tags/{}", rover_version)],
            &repo_path,
            None,
        )?;

        Ok(repo_path)
    }
}

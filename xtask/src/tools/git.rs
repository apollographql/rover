use crate::tools::Runner;
use std::{convert::TryFrom, fs};

use anyhow::{Context, Result};
use assert_fs::TempDir;
use camino::Utf8PathBuf;

const ROVER_DEFAULT_BRANCH: &str = "main";

pub(crate) struct GitRunner {
    runner: Runner,

    // we store _temp_dir here since its Drop implementation deletes the directory
    repo: RepoLocation,
}

#[allow(unused)]
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
    #[allow(unused)]
    pub(crate) fn new(path: &Utf8PathBuf) -> Result<Self> {
        let runner = Runner::new("git");
        Ok(GitRunner {
            runner,
            repo: RepoLocation::Local(LocalRepo { path: path.clone() }),
        })
    }
    pub(crate) fn tmp() -> Result<Self> {
        let runner = Runner::new("git");
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
        let url = format!("https://github.com/{org}/{name}");
        let path = self.get_path()?;
        if let RepoLocation::Local(local) = &self.repo {
            if fs::metadata(local.path.join(".git")).is_ok() {
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

    pub(crate) fn checkout_rover_version(&self, rover_version: &str) -> Result<Utf8PathBuf> {
        let repo_path = self.clone("apollographql", "rover", ROVER_DEFAULT_BRANCH)?;

        self.runner.exec(
            &["checkout", &format!("tags/{rover_version}")],
            &repo_path,
            None,
        )?;

        Ok(repo_path)
    }

    pub(crate) fn get_changed_files() -> Result<Vec<Utf8PathBuf>> {
        let apollo_main = GitRunner::tmp()?;

        let current_dir = std::env::current_dir()?;
        let current_dir = Utf8PathBuf::from_path_buf(current_dir).unwrap();

        let output = apollo_main.runner.exec(
            &[&format!("--work-tree={current_dir}"), "diff", "--name-only"],
            &current_dir,
            None,
        )?;
        Ok(output.stdout.split("\n").map(Utf8PathBuf::from).collect())
    }
}

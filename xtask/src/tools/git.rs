use std::convert::TryFrom;
use std::{env, fs};

use crate::tools::Runner;

use anyhow::{Context, Result};
use assert_fs::TempDir;
use camino::Utf8PathBuf;

pub(crate) struct GitRunner {
    repo: GitRepo,
    runner: Runner,
}

enum GitRepo {
    TempDir {
        temp_dir_path: Utf8PathBuf,

        // we store _temp_dir here since its Drop implementation deletes the directory
        _temp_dir: TempDir,
    },
    CiDir {
        ci_dir_path: Utf8PathBuf,
    },
}

impl GitRepo {
    fn try_new() -> Result<Self> {
        Ok(if env::var_os("CI").is_some() {
            let ci_dir_path = Utf8PathBuf::try_from(env::current_dir()?)?.join("test_tmp");
            fs::create_dir_all(&ci_dir_path)?;
            GitRepo::CiDir { ci_dir_path }
        } else {
            let temp_dir = TempDir::new().with_context(|| "Could not create temp directory")?;
            let temp_dir_path = Utf8PathBuf::try_from(temp_dir.path().to_path_buf())
                .with_context(|| "Temp directory was not valid Utf-8")?;
            GitRepo::TempDir {
                temp_dir_path,
                _temp_dir: temp_dir,
            }
        })
    }

    fn get_path(&self) -> &Utf8PathBuf {
        match self {
            GitRepo::TempDir {
                temp_dir_path,
                _temp_dir: _,
            } => temp_dir_path,
            GitRepo::CiDir { ci_dir_path } => ci_dir_path,
        }
    }
}

impl GitRunner {
    pub(crate) fn try_new(verbose: bool) -> Result<Self> {
        let runner = Runner::new("git", verbose)?;

        let repo = GitRepo::try_new()?;

        Ok(GitRunner { runner, repo })
    }

    pub(crate) fn clone_supergraph_demo(&self) -> Result<Utf8PathBuf> {
        let repo_name = "supergraph-demo";
        let repo_url = format!("https://github.com/apollographql/{}", repo_name);
        self.runner
            .exec(&["clone", &repo_url], self.repo.get_path(), None)?;

        let repo_path = self.repo.get_path().join(repo_name);
        Ok(repo_path)
    }

    pub(crate) fn checkout_rover_version(&self, rover_version: &str) -> Result<Utf8PathBuf> {
        let repo_name = "rover";
        let repo_url = format!("https://github.com/apollographql/{}", repo_name);
        let repo_path = self.repo.get_path();
        self.runner.exec(&["clone", &repo_url], repo_path, None)?;

        let repo_path = repo_path.join(repo_name);

        self.runner.exec(
            &["checkout", &format!("tags/{}", rover_version)],
            &repo_path,
            None,
        )?;

        Ok(repo_path)
    }
}

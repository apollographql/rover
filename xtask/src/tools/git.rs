use std::{convert::TryFrom, fs, str::FromStr};

use anyhow::{anyhow, Context, Result};
use assert_fs::TempDir;
use buildstructor::buildstructor;
use camino::Utf8PathBuf;
use url::Url;

use crate::tools::Runner;

const ROVER_DEFAULT_BRANCH: &str = "main";

pub(crate) struct GitRunner {
    runner: Runner,

    // we store _temp_dir here since its Drop implementation deletes the directory
    repo: RepoLocation,
    remote: GithubRepo,
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

#[derive(Debug, Clone)]
pub struct GithubRepo {
    org: String,
    name: String,
}

#[buildstructor]
impl GithubRepo {
    #[builder]
    pub fn new(org: Option<String>, name: Option<String>) -> GithubRepo {
        let org = org.unwrap_or_else(|| "apollographql".to_string());
        let name = name.unwrap_or_else(|| "rover".to_string());
        GithubRepo { org, name }
    }
}

impl Default for GithubRepo {
    fn default() -> Self {
        GithubRepo::builder().build()
    }
}

impl TryFrom<&GithubRepo> for Url {
    type Error = url::ParseError;
    fn try_from(value: &GithubRepo) -> Result<Self, Self::Error> {
        Url::parse(&format!("https://github.com/{}/{}", value.org, value.name))
    }
}

impl FromStr for GithubRepo {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let url = Url::parse(s).map_err(|err| anyhow!("Invalid Url. {:?}", err))?;
        let mut path = url
            .path_segments()
            .ok_or_else(|| anyhow!("Could not extract path from Url"))?;
        let org = path
            .next()
            .ok_or_else(|| anyhow!("Could not extract org from Url path"))?;
        let name = path
            .next()
            .ok_or_else(|| anyhow!("Could not extract repo from Url path"))?;
        Ok(GithubRepo {
            org: org.to_string(),
            name: name.to_string(),
        })
    }
}

impl GitRunner {
    pub(crate) fn new(remote: GithubRepo, path: &Utf8PathBuf) -> Result<Self> {
        let runner = Runner::new("git");
        Ok(GitRunner {
            runner,
            remote,
            repo: RepoLocation::Local(LocalRepo { path: path.clone() }),
        })
    }
    pub(crate) fn tmp(remote: GithubRepo) -> Result<Self> {
        let runner = Runner::new("git");
        let temp_dir = TempDir::new().with_context(|| "Could not create temp directory")?;
        let temp_dir_path = Utf8PathBuf::try_from(temp_dir.path().to_path_buf())
            .with_context(|| "Temp directory was not valid Utf-8")?;

        Ok(GitRunner {
            remote,
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

    pub fn clone(&self, branch: &str) -> Result<Utf8PathBuf> {
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

        let url = Url::try_from(&self.remote)?;

        self.runner.exec(
            &["clone", url.as_str(), "--branch", branch, path.as_ref()],
            &crate::utils::PKG_PROJECT_ROOT,
            None,
        )?;

        Ok(path)
    }

    pub(crate) fn checkout_rover_version(&self, rover_version: &str) -> Result<Utf8PathBuf> {
        let repo_path = self.clone(ROVER_DEFAULT_BRANCH)?;

        self.runner.exec(
            &["checkout", &format!("tags/{}", rover_version)],
            &repo_path,
            None,
        )?;

        Ok(repo_path)
    }

    pub(crate) fn checkout_rover_sha(&self, sha: &str) -> Result<Utf8PathBuf> {
        let repo_path = self.clone(ROVER_DEFAULT_BRANCH)?;

        self.runner.exec(&["checkout", sha], &repo_path, None)?;

        Ok(repo_path)
    }

    pub(crate) fn get_changed_files(&self, current_sha: &str) -> Result<Vec<Utf8PathBuf>> {
        let repo_path = self.checkout_rover_sha(current_sha)?;

        let is_default_branch = self
            .runner
            .exec(
                &["cherry", "-v", ROVER_DEFAULT_BRANCH, current_sha],
                &repo_path,
                None,
            )?
            .stdout
            .is_empty();

        let base_sha = if is_default_branch {
            self.runner
                .exec(
                    &["rev-parse", &format!("{}~1", ROVER_DEFAULT_BRANCH)],
                    &repo_path,
                    None,
                )?
                .stdout
        } else {
            let list_output = self
                .runner
                .exec(
                    &[
                        "rev-list",
                        "--boundary",
                        &format!("{}...{}", current_sha, ROVER_DEFAULT_BRANCH),
                    ],
                    &repo_path,
                    None,
                )?
                .stdout;
            // Process the output, split it, find the line that starts with a `-` and then
            // extract the commit contained in that line
            let base_sha = list_output
                .split("\n")
                .find(|l| l.starts_with("-"))
                .ok_or(anyhow!("could not find base commit"))?;
            base_sha[1..base_sha.len()].to_string()
        };

        let output = self.runner.exec(
            &["diff", "--name-only", current_sha, &base_sha],
            &repo_path,
            None,
        )?;
        Ok(output.stdout.split("\n").map(Utf8PathBuf::from).collect())
    }
}

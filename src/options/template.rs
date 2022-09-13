use std::fmt;
use std::fmt::Display;

use saucer::{anyhow, clap, Parser, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Parser)]
pub struct TemplateOpt {
    /// Filter templates by the available language
    #[clap(long = "language", value_enum)]
    pub language: Option<ProjectLanguage>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct GithubTemplate {
    pub id: &'static str,
    pub git_url: &'static str,
    pub display: &'static str,
    pub language: ProjectLanguage,
}

impl GithubTemplate {
    pub(crate) fn repo_slug(&self) -> Result<&'static str> {
        self.git_url
            .split('/')
            .last()
            .ok_or_else(|| anyhow!("Could not determine tarball path."))
    }
}

impl Display for GithubTemplate {
    // This trait requires `fmt` with this exact signature.
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // Write strictly the first element into the supplied output
        // stream: `f`. Returns `fmt::Result` which indicates whether the
        // operation succeeded or failed. Note that `write!` uses syntax which
        // is very similar to `println!`.
        write!(f, "{}", self.display)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, clap::ValueEnum)]
pub enum ProjectLanguage {
    Java,
    Javascript,
    Python,
    Rust,
    Typescript,
}

impl Display for ProjectLanguage {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ProjectLanguage::Java => write!(f, "java"),
            ProjectLanguage::Javascript => write!(f, "javascript"),
            ProjectLanguage::Python => write!(f, "python"),
            ProjectLanguage::Rust => write!(f, "rust"),
            ProjectLanguage::Typescript => write!(f, "typescript"),
        }
    }
}

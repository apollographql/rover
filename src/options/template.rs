use saucer::{clap, Parser};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::fmt::Display;

#[derive(Debug, Clone, Serialize, Deserialize, Parser)]
pub struct TemplateOpt {
    /// The ID for the official template to use
    #[clap(short = 't', long = "template")]
    #[serde(skip_serializing)]
    pub template: Option<String>,

    /// Filter templates by the available language
    #[clap(long = "language", value_enum)]
    #[serde(skip_serializing)]
    pub language: Option<ProjectLanguage>,

    /// Type of template project: client or subgraph
    #[clap(long = "project-type", value_enum)]
    #[serde(skip_serializing)]
    pub project_type: Option<ProjectType>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GithubTemplate {
    pub id: &'static str,
    pub git_url: &'static str,
    pub display: &'static str,
    pub language: ProjectLanguage,
    pub project_type: ProjectType,
}

impl fmt::Display for GithubTemplate {
    // This trait requires `fmt` with this exact signature.
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // Write strictly the first element into the supplied output
        // stream: `f`. Returns `fmt::Result` which indicates whether the
        // operation succeeded or failed. Note that `write!` uses syntax which
        // is very similar to `println!`.
        write!(f, "{}", self.display)
    }
}

#[derive(Clone, Copy, Deserialize, Debug, Eq, PartialEq, Serialize, clap::ValueEnum)]
pub enum ProjectType {
    Subgraph,
    Client,
}

impl Display for ProjectType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ProjectType::Subgraph => write!(f, "subgraph"),
            ProjectType::Client => write!(f, "client"),
        }
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

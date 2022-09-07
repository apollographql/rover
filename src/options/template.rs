use crate::anyhow;
use saucer::{clap, Parser};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Serialize, Deserialize, Parser)]
pub struct TemplateOpt {
    /// The GitHub id for the templates repository
    #[clap(short = 't', long = "template")]
    #[serde(skip_serializing)]
    pub template: Option<String>,

    /// Filter templates by the available language
    #[clap(long = "language")]
    #[serde(skip_serializing)]
    pub language: Option<String>,

    /// Type of template project: client or subgraph
    #[clap(long = "project-type")]
    #[serde(skip_serializing)]
    pub project_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Parser)]
pub struct GithubTemplate {
    pub id: String,
    pub git_url: String,
    pub display: String,
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

#[derive(Debug, Clone, Serialize, Deserialize, Parser)]
pub enum ProjectType {
    All,
    Subgraph,
    Client,
}

impl PartialEq<String> for ProjectType {
    fn eq(&self, other: &String) -> bool {
        match &self {
            ProjectType::All => other.to_lowercase() == "all",
            ProjectType::Subgraph => other.to_lowercase() == "subgraph",
            ProjectType::Client => other.to_lowercase() == "client",
        }
    }
}
impl PartialEq for ProjectType {
    fn eq(&self, other: &ProjectType) -> bool {
        match &self {
            ProjectType::All => other.eq(&String::from("all")),
            ProjectType::Subgraph => other.eq(&String::from("subgraph")),
            ProjectType::Client => other.eq(&String::from("client")),
        }
    }
}
impl fmt::Display for ProjectType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ProjectType::All => write!(f, "all"),
            ProjectType::Subgraph => write!(f, "subgraph"),
            ProjectType::Client => write!(f, "client"),
        }
    }
}

impl FromStr for ProjectType {
    type Err = saucer::Error;

    fn from_str(input: &str) -> std::result::Result<Self, Self::Err> {
        match input.to_lowercase().as_str() {
            "all" => Ok(ProjectType::All),
            "subgraph" => Ok(ProjectType::Subgraph),
            "client" => Ok(ProjectType::Client),
            _ => Err(anyhow!("Invalid Project Type")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Parser)]
pub enum ProjectLanguage {
    Java,
    Javascript,
    Python,
    Rust,
    Typescript,
}

impl PartialEq<String> for ProjectLanguage {
    fn eq(&self, other: &String) -> bool {
        match &self {
            ProjectLanguage::Java => other.to_lowercase() == "java",
            ProjectLanguage::Javascript => other.to_lowercase() == "javascript",
            ProjectLanguage::Python => other.to_lowercase() == "python",
            ProjectLanguage::Rust => other.to_lowercase() == "rust",
            ProjectLanguage::Typescript => other.to_lowercase() == "typescript",
        }
    }
}
impl PartialEq for ProjectLanguage {
    fn eq(&self, other: &ProjectLanguage) -> bool {
        match &self {
            ProjectLanguage::Java => other.eq(&String::from("java")),
            ProjectLanguage::Javascript => other.eq(&String::from("javascript")),
            ProjectLanguage::Python => other.eq(&String::from("python")),
            ProjectLanguage::Rust => other.eq(&String::from("rust")),
            ProjectLanguage::Typescript => other.eq(&String::from("typescript")),
        }
    }
}
impl fmt::Display for ProjectLanguage {
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

impl FromStr for ProjectLanguage {
    type Err = saucer::Error;

    fn from_str(input: &str) -> std::result::Result<Self, Self::Err> {
        match input.to_lowercase().as_str() {
            "java" => Ok(ProjectLanguage::Java),
            "javascript" => Ok(ProjectLanguage::Javascript),
            "python" => Ok(ProjectLanguage::Python),
            "rust" => Ok(ProjectLanguage::Rust),
            "typescript" => Ok(ProjectLanguage::Typescript),
            _ => Err(anyhow!("Invalid Project Type")),
        }
    }
}

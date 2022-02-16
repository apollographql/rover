use std::{
    fmt::{self, Display},
    str::FromStr,
};

use anyhow::Context;
use camino::Utf8PathBuf;
use serde::Serialize;
use strum::IntoEnumIterator;
use strum_macros::EnumIter;

use crate::anyhow;
use crate::error::RoverError;

#[derive(Debug, Clone, Serialize, EnumIter)]
pub(crate) enum Template {
    Rust,
    Typescript,
}

impl Template {
    pub(crate) fn clone_repo(&self, directory: &Utf8PathBuf) -> Result<(), RoverError> {
        if directory.exists() {
            return Err(RoverError::new(anyhow!(
                "`{}` already exists, please select a different directory for your project, or remove it.",
                directory
            )));
        }

        let repo_url = self.get_repository_url();

        std::process::Command::new("git")
            .args(&["clone", repo_url, directory.as_str()])
            .output()
            .with_context(|| format!("Could not clone `{}` into `{}`", repo_url, directory))?;
        Ok(())
        // Ok(Repository::clone(repo_url, directory)
        //     .with_context(|| format!("Could not clone `{}` into `{}`", repo_url, directory))?)
    }
    pub(crate) fn get_repository_url(&self) -> &str {
        match self {
            Self::Rust => "https://github.com/apollographql/supergraph-rs",
            Self::Typescript => "https://github.com/apollographql/fed-day-1",
        }
    }
}

impl Template {
    pub(crate) fn possible_templates() -> Vec<&'static str> {
        let mut res = Vec::new();
        for lang in Self::iter() {
            res.push(lang.as_str());
        }
        res
    }

    pub(crate) fn as_str(&self) -> &'static str {
        match &self {
            Self::Rust => "rust",
            Self::Typescript => "typescript",
        }
    }
}

impl FromStr for Template {
    type Err = RoverError;
    fn from_str(lang: &str) -> Result<Self, Self::Err> {
        match lang.to_lowercase().as_str() {
            "rust" => Ok(Self::Rust),
            "typescript" => Ok(Self::Typescript),
            _ => Err(RoverError::new(anyhow!(
                "Could not find a template for language `{}`.",
                lang
            ))),
        }
    }
}

impl Display for Template {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let template = self.as_str();
        write!(f, "{}", template)
    }
}

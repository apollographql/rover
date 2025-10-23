use std::{fmt::Display, str::FromStr};

use serde_with::{DeserializeFromStr, SerializeDisplay};

use crate::command::install::plugin::{PluginVersion, error::PluginError};

/// An MCP Server version.
#[derive(Debug, Clone, SerializeDisplay, DeserializeFromStr, PartialEq, Eq)]
pub enum Version {
    /// An exact MCP Server version
    Exact(semver::Version),

    /// The latest MCP Server version
    Latest,
}

impl PluginVersion for Version {
    fn get_major_version(&self) -> u64 {
        match self {
            Version::Exact(v) => v.major,
            Version::Latest => 0,
        }
    }

    fn get_tarball_version(&self) -> String {
        match self {
            Version::Exact(v) => format!("v{v}"),
            Version::Latest => "latest".to_string(),
        }
    }
}

impl Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Version::Exact(v) => write!(f, "={v}"),
            Version::Latest => write!(f, "latest"),
        }
    }
}

impl FromStr for Version {
    type Err = PluginError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s == "latest" {
            return Ok(Version::Latest);
        }

        if !s.starts_with('=') && !s.starts_with('v') {
            return Err(PluginError::InvalidVersionFormat(format!(
                "Specified version `{s}` is not supported. You can specify 'latest' or a fully qualified version prefixed with an '=', like: =1.0.0"
            )));
        }

        semver::Version::parse(&s[1..])
            .map(Version::Exact)
            .map_err(|_| PluginError::InvalidVersionFormat(format!("Specified version `{s}` is not supported. You can specify 'latest' or a fully qualified version prefixed with an '=', like: =1.0.0")))
    }
}

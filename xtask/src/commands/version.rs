use std::{fmt, str::FromStr};

use anyhow::{anyhow, Result};
use semver::Version;

#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub(crate) struct RoverVersion {
    inner: Version,
}

impl FromStr for RoverVersion {
    type Err = anyhow::Error;

    fn from_str(proposed_version: &str) -> Result<Self, Self::Err> {
        if proposed_version.is_empty() {
            Err(anyhow!("version cannot be empty"))
        } else {
            let mut version_chars = proposed_version.chars();

            // check `v` prefix exists, and strip it from the input string
            if version_chars.next().unwrap() != 'v' {
                Err(anyhow!("version must start with `v`"))
            } else {
                let version = Version::parse(version_chars.as_str())?;
                let min_supported_version = Version::new(0, 1, 3);
                if version < min_supported_version {
                    Err(anyhow!("version must be >= {}", min_supported_version))
                } else {
                    Ok(RoverVersion { inner: version })
                }
            }
        }
    }
}

impl fmt::Display for RoverVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "v{}", self.inner)
    }
}

use std::str::FromStr;

use apollo_federation_types::config::FederationVersion;
use serde::{Deserialize, Serialize};

use crate::{anyhow, Context, Result};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) enum Plugin {
    Supergraph(FederationVersion),
}

impl Plugin {
    pub fn get_name(&self) -> String {
        match self {
            Self::Supergraph(_) => "supergraph".to_string(),
        }
    }

    pub fn get_major_version(&self) -> u64 {
        match self {
            Self::Supergraph(v) => v.get_major_version(),
        }
    }

    pub fn requires_elv2_license(&self) -> bool {
        match self {
            Self::Supergraph(v) => v.get_major_version() == 2,
        }
    }

    pub fn get_tarball_url(&self) -> Result<String> {
        match self {
            Self::Supergraph(v) => {
                let target_arch = if cfg!(target_os = "windows") {
                    Ok("x86_64-pc-windows-msvc")
                } else if cfg!(target_os = "macos") {
                    Ok("x86_64-apple-darwin")
                } else if cfg!(target_os = "linux") && !cfg!(target_env = "musl") {
                    Ok("x86_64-unknown-linux-gnu")
                } else {
                    Err(anyhow!(
                        "Your current architecture does not support installation of this plugin."
                    ))
                }?;
                Ok(format!(
                    "https://rover.apollo.dev/tar/{name}/{target_arch}/{version}",
                    name = self.get_name(),
                    target_arch = target_arch,
                    version = v.get_tarball_version()
                ))
            }
        }
    }
}

impl FromStr for Plugin {
    type Err = saucer::Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let lowercase = s.to_lowercase();
        let splits: Vec<String> = lowercase.split('@').map(|x| x.to_string()).collect();
        if splits.len() == 2 {
            let plugin_name = splits[0].clone();
            let plugin_version = splits[1].clone();
            if plugin_name == "supergraph" {
                let federation_version = FederationVersion::from_str(&plugin_version).with_context(||
                    format!("Invalid version '{}' for 'supergraph' plugin. Must be 'latest-0', 'latest-2', or an exact version preceeded with an '='.", &plugin_version))?;
                Ok(Plugin::Supergraph(federation_version))
            } else {
                Err(anyhow!(
                    "Invalid plugin name {}. Possible values are [supergraph].",
                    plugin_name
                ))
            }
        } else {
            Err(anyhow!("Plugin must be in form '{{name}}@{{version}}'."))
        }
    }
}

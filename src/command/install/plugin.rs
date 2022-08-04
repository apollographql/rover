use std::str::FromStr;

use apollo_federation_types::config::FederationVersion;
use serde::{Deserialize, Serialize};

use crate::{anyhow, Context, Result};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) enum Plugin {
    Supergraph(FederationVersion),
    Router,
}

impl Plugin {
    pub fn get_name(&self) -> String {
        match self {
            Self::Supergraph(_) => "supergraph".to_string(),
            Self::Router => "router".to_string(),
        }
    }

    pub fn get_major_version(&self) -> u64 {
        match self {
            Self::Supergraph(v) => v.get_major_version(),
            // TODO: replace this with real versioning
            Self::Router => 0,
        }
    }

    pub fn requires_elv2_license(&self) -> bool {
        match self {
            Self::Supergraph(v) => v.get_major_version() == 2,
            Self::Router => true,
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
            Self::Router => {
                let target_arch = if !cfg!(target_arch = "x86_64") && !cfg!(target_os = "macos") {
                    Err(anyhow!(
                        "Your current architecture does not support installation of this plugin."
                    ))
                } else if cfg!(target_os = "windows") {
                    Ok("x86_64-windows")
                } else if cfg!(target_os = "macos") {
                    Ok("x86_64-macos")
                } else if cfg!(target_os = "linux") && !cfg!(target_env = "musl") {
                    Ok("x86_64-linux")
                } else {
                    Err(anyhow!(
                        "Your current architecture does not support installation of this plugin."
                    ))
                }?;
                Ok(format!(
                    // TODO: replace this with real versions, probably sourced from orbiter
                    "https://github.com/apollographql/{name}/releases/download/v0.12.0/{name}-0.12.0-{target_arch}.tar.gz",
                    name = self.get_name(),
                    target_arch = target_arch,
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
            } else if plugin_name == "router" {
                // TODO: real versioning
                eprintln!(
                    "warn: router version {} has been ignored, using 0.12.0 instead",
                    &plugin_version
                );
                Ok(Plugin::Router)
            } else {
                // TODO: this should probably use ArgEnum instead
                Err(anyhow!(
                    "Invalid plugin name {}. Possible values are [supergraph, router].",
                    plugin_name
                ))
            }
        } else {
            Err(anyhow!("Plugin must be in form '{{name}}@{{version}}'."))
        }
    }
}

use std::str::FromStr;

use apollo_federation_types::config::{FederationVersion, PluginVersion, RouterVersion};
use serde::{Deserialize, Serialize};

use crate::{anyhow, error::RoverError, Context, Result};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) enum Plugin {
    Supergraph(FederationVersion),
    Router(RouterVersion),
}

impl Plugin {
    pub fn get_name(&self) -> String {
        match self {
            Self::Supergraph(_) => "supergraph".to_string(),
            Self::Router(_) => "router".to_string(),
        }
    }

    pub fn requires_elv2_license(&self) -> bool {
        match self {
            Self::Supergraph(v) => v.get_major_version() == 2,
            Self::Router(_) => true,
        }
    }

    pub fn get_tarball_version(&self) -> String {
        match self {
            Self::Supergraph(v) => v.get_tarball_version(),
            Self::Router(v) => v.get_tarball_version(),
        }
    }

    pub fn get_target_arch(&self) -> Result<String> {
        if cfg!(target_os = "windows") {
            Ok("x86_64-pc-windows-msvc")
        } else if cfg!(target_os = "macos") {
            Ok("x86_64-apple-darwin")
        } else if cfg!(target_os = "linux") && !cfg!(target_env = "musl") {
            Ok("x86_64-unknown-linux-gnu")
        } else {
            Err(RoverError::new(anyhow!(
                "Your current architecture does not support installation of this plugin."
            )))
        }
        .map(|s| s.to_string())
    }

    pub fn get_tarball_url(&self) -> Result<String> {
        Ok(format!(
            "https://rover.apollo.dev/tar/{name}/{target_arch}/{version}",
            name = self.get_name(),
            target_arch = self.get_target_arch()?,
            version = self.get_tarball_version()
        ))
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
                let federation_version = FederationVersion::from_str(&plugin_version)
                    .with_context(|| {
                        format!(
                            "Invalid version '{}' for 'supergraph' plugin. Must be 'latest-0', 'latest-2', or an exact version preceeded with an '='.",
                            &plugin_version
                        )
                    })?;
                Ok(Plugin::Supergraph(federation_version))
            } else if plugin_name == "router" {
                let router_version = RouterVersion::from_str(&plugin_version).with_context({
                    || format!("Invalid version '{}' for 'router' plugin. Must be an exact version (>= 1.0.0 & < 2.0.0) preceeded with a 'v'.", &plugin_version)
                })?;
                Ok(Plugin::Router(router_version))
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

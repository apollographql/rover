use std::env::consts;
use std::str::FromStr;

use apollo_federation_types::config::FederationVersion;
use serde::{Deserialize, Serialize};

use crate::{
    anyhow,
    error::{RoverError, Suggestion},
    Context, Result,
};

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
                let no_prebuilt_binaries = anyhow!(
                    "Your current architecture does not support installation of this plugin."
                );
                // Sorry, no musl support for composition
                if cfg!(target_env = "musl") {
                    let mut e = RoverError::new(no_prebuilt_binaries);
                    e.set_suggestion(Suggestion::CheckGnuVersion);
                    return Err(e);
                }
                let target_arch = match (consts::OS, consts::ARCH) {
                    ("windows", _) => Ok("x86_64-pc-windows-msvc"),
                    ("macos", "x86_64") => Ok("x86_64-apple-darwin"),
                    ("macos", "aarch64") => {
                        // we didn't always build aarch64 MacOS binaries,
                        // so check to see if this version supports them or not
                        if v.supports_arm() {
                            Ok("aarch64-apple-darwin")
                        } else {
                            // if an old version doesn't have aarch64 binaries,
                            // download the x86_64 versions
                            // this should work because of Apple's Rosetta 2 emulation software
                            Ok("x86_64-apple-darwin")
                        }
                    }
                    ("linux", "x86_64") => Ok("x86-64-unknown-linux-gnu"),
                    ("linux", "aarch64") => {
                        if v.supports_arm() {
                            // we didn't always build aarch64 linux binaries,
                            // so check to see if this version supports them or not
                            Ok("aarch64-unknown-linux-gnu")
                        } else {
                            // if an old version doesn't have aarch64 binaries,
                            // you're out of luck
                            Err(no_prebuilt_binaries)
                        }
                    }
                    _ => Err(no_prebuilt_binaries),
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
    type Err = anyhow::Error;

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

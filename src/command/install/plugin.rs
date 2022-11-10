use std::{env::consts, str::FromStr};

use anyhow::{anyhow, Context};
use apollo_federation_types::config::{FederationVersion, PluginVersion, RouterVersion};
use semver::Version;
use serde::{Deserialize, Serialize};

use crate::{RoverError, RoverErrorSuggestion, RoverResult};

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

    pub fn get_target_arch(&self) -> RoverResult<String> {
        let mut no_prebuilt_binaries = RoverError::new(anyhow!(
            "Your current architecture does not support installation of this plugin."
        ));
        // Sorry, no musl support for composition or the router
        if cfg!(target_env = "musl") {
            no_prebuilt_binaries.set_suggestion(RoverErrorSuggestion::CheckGnuVersion);
            return Err(no_prebuilt_binaries);
        }

        match (consts::OS, consts::ARCH) {
            ("windows", _) => Ok("x86_64-pc-windows-msvc"),
            ("macos", _) => Ok("x86_64-apple-darwin"),
            ("linux", "x86_64") => Ok("x86_64-unknown-linux-gnu"),
            ("linux", "aarch64") => {
                match self {
                    Self::Supergraph(v) => {
                        if v.supports_arm_linux() {
                            // we didn't always build aarch64 linux binaries,
                            // so check to see if this version supports them or not
                            Ok("aarch64-unknown-linux-gnu")
                        } else {
                            // if an old version doesn't have aarch64 binaries,
                            // you're out of luck
                            if v.is_fed_one() {
                                no_prebuilt_binaries.set_suggestion(RoverErrorSuggestion::Adhoc("Newer versions of this plugin have prebuilt binaries for this architecture, if you set `federation_version: 1` in your `supergraph.yaml`, it should automatically update to a supported version.".to_string()))
                            } else if v.is_fed_two() {
                                no_prebuilt_binaries.set_suggestion(RoverErrorSuggestion::Adhoc("Newer versions of this plugin have prebuilt binaries for this architecture, if you set `federation_version: 2` in your `supergraph.yaml`, it should automatically update to a supported version.".to_string()))
                            }
                            Err(no_prebuilt_binaries)
                        }
                    },
                    Self::Router(v) => {
                        match v {
                            RouterVersion::Exact(v) => {
                                if v >= &Version::new(1, 1, 0) {
                                    Ok("aarch64-unknown-linux-gnu")
                                } else {
                                    no_prebuilt_binaries.set_suggestion(RoverErrorSuggestion::Adhoc("Newer versions of this plugin have prebuilt binaries for this architecture.".to_string()));
                                    Err(no_prebuilt_binaries)
                                }
                            }
                            RouterVersion::Latest => Ok("aarch64-unknown-linux-gnu")
                        }
                    }
                }
            }
            _ => Err(no_prebuilt_binaries),
        }
        .map(|s| s.to_string())
    }

    pub fn get_tarball_url(&self) -> RoverResult<String> {
        Ok(format!(
            "https://rover.apollo.dev/tar/{name}/{target_arch}/{version}",
            name = self.get_name(),
            target_arch = self.get_target_arch()?,
            version = self.get_tarball_version()
        ))
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

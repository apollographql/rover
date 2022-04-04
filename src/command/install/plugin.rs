use std::str::FromStr;

use serde::{Deserialize, Serialize};
use structopt::StructOpt;

use crate::{anyhow, Result};

#[derive(StructOpt, Clone, Copy, Debug, Serialize, Deserialize)]
#[structopt(rename_all = "kebab-case")]
pub(crate) enum Plugin {
    Supergraph0,
    Supergraph2,
}

impl Plugin {
    pub fn get_name(&self) -> String {
        match self {
            Self::Supergraph0 | Self::Supergraph2 => "supergraph".to_string(),
        }
    }

    pub fn get_latest(&self) -> String {
        format!("latest-{}", &self.get_major())
    }

    pub fn get_major(&self) -> String {
        match self {
            Self::Supergraph0 => "0",
            Self::Supergraph2 => "2",
        }
        .to_string()
    }

    pub fn get_tarball_url(&self) -> Result<String> {
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
            version = self.get_latest()
        ))
    }
}

impl FromStr for Plugin {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let lowercase = s.to_lowercase();
        match lowercase.as_str() {
            "supergraph-0" => Ok(Plugin::Supergraph0),
            "supergraph-2" => Ok(Plugin::Supergraph2),
            _ => Err(anyhow::anyhow!("Invalid plugin name {}.", s)),
        }
    }
}

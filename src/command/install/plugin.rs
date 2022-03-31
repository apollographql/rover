use std::str::FromStr;

use serde::{Deserialize, Serialize};
use structopt::StructOpt;

#[derive(StructOpt, Debug, Serialize, Deserialize)]
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
        match self {
            Self::Supergraph0 => "latest-0".to_string(),
            Self::Supergraph2 => "latest-2".to_string(),
        }
    }

    pub fn get_tarball_url(&self, target_arch: &str) -> String {
        format!(
            "https://rover.apollo.dev/tar/{name}/{target_arch}/{version}",
            name = self.get_name(),
            target_arch = target_arch,
            version = self.get_latest()
        )
    }
}

impl FromStr for Plugin {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let lowercase = s.to_lowercase();
        match lowercase.as_str() {
            "supergraph-0" => Ok(Plugin::Supergraph0),
            "supergraph-2" => Ok(Plugin::Supergraph2),
            _ => Err(anyhow::anyhow!("Invalid plugin name.")),
        }
    }
}

use std::str::FromStr;

use serde::{Deserialize, Serialize};
use structopt::StructOpt;

use crate::PKG_VERSION;

#[derive(StructOpt, Debug, Serialize, Deserialize)]
#[structopt(rename_all = "kebab-case")]
pub(crate) enum Plugin {
    Supergraph,
}

impl Plugin {
    pub fn get_name(&self) -> String {
        match self {
            Self::Supergraph => "supergraph".to_string(),
        }
    }

    pub fn get_tarball_url(&self, target_arch: &str) -> String {
        match self {
            Self::Supergraph => format!(
                "https://github.com/apollographql/federation-rs/releases/download/{name}%40v{version}/{}-v{version}-{target_arch}.tar.gz",
                version = PKG_VERSION,
                name = self.get_name(),
                target_arch = target_arch
            )
        }
    }
}

impl FromStr for Plugin {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let lowercase = s.to_lowercase();
        match lowercase.as_str() {
            "supergraph" => Ok(Plugin::Supergraph),
            _ => Err(anyhow::anyhow!("Invalid plugin name.")),
        }
    }
}

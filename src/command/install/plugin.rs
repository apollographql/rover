use std::str::FromStr;

use serde::{Deserialize, Serialize};
use structopt::StructOpt;

use crate::PKG_VERSION;

#[derive(StructOpt, Debug, Serialize, Deserialize)]
#[structopt(rename_all = "kebab-case")]
pub(crate) enum Plugin {
    RoverFed,
}

impl Plugin {
    pub fn get_name(&self) -> String {
        match self {
            Self::RoverFed => "rover-fed2".to_string(),
        }
    }

    pub fn get_tarball_url(&self, target_arch: &str) -> String {
        format!(
            "https://github.com/apollographql/rover/releases/download/v{}/{}-v{}-{}.tar.gz",
            PKG_VERSION,
            self.get_name(),
            PKG_VERSION,
            target_arch
        )
    }
}

impl FromStr for Plugin {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let lowercase = s.to_lowercase();
        match lowercase.as_str() {
            "rover-fed2" => Ok(Plugin::RoverFed),
            _ => Err(anyhow::anyhow!("Invalid plugin name.")),
        }
    }
}

use std::str::FromStr;

use serde::{Deserialize, Serialize};
use structopt::StructOpt;

#[derive(StructOpt, Debug, Serialize, Deserialize)]
#[structopt(rename_all = "kebab-case")]
pub(crate) enum Plugin {
    RoverFed,
}

impl Plugin {
    pub fn get_name(&self) -> String {
        match self {
            Self::RoverFed => "rover-fed".to_string(),
        }
    }

    pub fn get_tarball_url(&self) -> String {
        match self {
          // TODO: make a url automatically by calling self.get_name()
          // also probably need a way to override the version
          Self::RoverFed => "https://github.com/apollographql/rover/releases/download/v0.2.0/rover-v0.2.0-x86_64-unknown-linux-gnu.tar.gz".to_string()
        }
    }
}

impl FromStr for Plugin {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let lowercase = s.to_lowercase();
        match lowercase.as_str() {
            "rover-fed" => Ok(Plugin::RoverFed),
            _ => Err(anyhow::anyhow!("Invalid plugin name.")),
        }
    }
}

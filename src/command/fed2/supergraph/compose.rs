use crate::utils::client::StudioClientConfig;
use crate::Suggestion;
use crate::{anyhow, command::RoverOutput, error::RoverError, Result};

use camino::Utf8PathBuf;
use serde::Serialize;
use structopt::StructOpt;

use std::str;

#[derive(Debug, Serialize, StructOpt)]
pub struct Compose {
    /// The relative path to the supergraph configuration file.
    #[structopt(long = "config")]
    #[serde(skip_serializing)]
    config_path: Utf8PathBuf,

    /// Name of configuration profile to use
    #[structopt(long = "profile", default_value = "default")]
    #[serde(skip_serializing)]
    profile_name: String,
}

impl Compose {
    pub fn run(&self, client_config: StudioClientConfig) -> Result<RoverOutput> {
        let mut err = RoverError::new(anyhow!("This command has been deprecated."));
        err.set_suggestion(Suggestion::Adhoc(format!(
            "Please set `federation_version = 2` in `{}` and run `rover supergraph compose`",
            &self.config_path
        )));
        Err(err)
    }
}

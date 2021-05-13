use camino::Utf8PathBuf;
use serde::Serialize;
use structopt::StructOpt;

use crate::utils::client::StudioClientConfig;
use crate::{
    anyhow,
    command::RoverStdout,
    error::{RoverError, Suggestion},
    Result,
};

use std::path::Path;

#[derive(Debug, Serialize, StructOpt)]
pub struct Compose {
    /// The relative path to the supergraph configuration file.
    #[structopt(long = "config")]
    #[serde(skip_serializing)]
    config_path: Option<Utf8PathBuf>,

    /// Name of configuration profile to use
    #[structopt(long = "profile", default_value = "default")]
    #[serde(skip_serializing)]
    profile_name: String,
}

impl Compose {
    pub fn run(&self, _client_config: StudioClientConfig) -> Result<RoverStdout> {
        let mut err = RoverError::new(anyhow!(
            "This version of Rover does not support this command."
        ));
        err.set_suggestion(Suggestion::CheckGnuVersion);
        Err(err)
    }
}

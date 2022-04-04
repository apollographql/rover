use camino::Utf8PathBuf;
use serde::Serialize;
use structopt::StructOpt;

use crate::utils::client::StudioClientConfig;
use crate::{
    anyhow,
    command::RoverOutput,
    error::{RoverError, Suggestion},
    Result,
};

#[derive(Debug, Serialize, StructOpt)]
pub struct Compose {
    /// The relative path to the supergraph configuration file.
    #[structopt(long = "config")]
    #[serde(skip_serializing)]
    #[allow(unused)]
    config_path: Option<Utf8PathBuf>,

    /// Name of configuration profile to use
    #[structopt(long = "profile", default_value = "default")]
    #[serde(skip_serializing)]
    #[allow(unused)]
    profile_name: String,
}

impl Compose {
    pub fn run(
        &self,
        _override_install_path: Option<Utf8PathBuf>,
        _client_config: StudioClientConfig,
    ) -> Result<RoverOutput> {
        let mut err = RoverError::new(anyhow!(
            "This version of Rover does not support this command."
        ));
        err.set_suggestion(Suggestion::CheckGnuVersion);
        Err(err)
    }
}

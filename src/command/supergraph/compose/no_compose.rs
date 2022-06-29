use camino::Utf8PathBuf;
use serde::Serialize;
use structopt::StructOpt;

use crate::options::ProfileOpt;
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

    #[structopt(flatten)]
    #[allow(unused)]
    profile: ProfileOpt,
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

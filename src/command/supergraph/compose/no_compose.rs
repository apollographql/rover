use camino::Utf8PathBuf;
use clap::Parser;
use serde::Serialize;

use crate::options::ProfileOpt;
use crate::utils::client::StudioClientConfig;
use crate::{
    anyhow,
    error::{RoverError, Suggestion},
    Result, RoverOutput,
};

#[derive(Debug, Serialize, Parser)]
pub struct Compose {
    /// The relative path to the supergraph configuration file.
    #[clap(long = "config")]
    #[serde(skip_serializing)]
    #[allow(unused)]
    config_path: Option<Utf8PathBuf>,

    #[clap(flatten)]
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

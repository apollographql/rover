use anyhow::anyhow;
use camino::Utf8PathBuf;
use clap::Parser;
use serde::Serialize;

use crate::{
    RoverError, RoverErrorSuggestion, RoverOutput, RoverResult, options::ProfileOpt,
    utils::client::StudioClientConfig,
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
    pub async fn run(
        &self,
        _override_install_path: Option<Utf8PathBuf>,
        _client_config: StudioClientConfig,
        _output_file: Option<Utf8PathBuf>,
    ) -> RoverResult<RoverOutput> {
        let mut err = RoverError::new(anyhow!(
            "This version of Rover does not support this command."
        ));
        err.set_suggestion(RoverErrorSuggestion::CheckGnuVersion);
        Err(err)
    }
}

use crate::{
    options::ProfileOpt,
    utils::{client::StudioClientConfig, parsers::FileDescriptorType},
};
use crate::{RoverError, RoverErrorSuggestion, RoverOutput, RoverResult};

use anyhow::anyhow;
use clap::Parser;
use serde::Serialize;

#[derive(Debug, Serialize, Parser)]
pub struct Compose {
    /// The relative path to the supergraph configuration file. You can pass `-` to use stdin instead of a file.
    #[arg(long = "config")]
    #[serde(skip_serializing)]
    supergraph_yaml: FileDescriptorType,

    #[clap(flatten)]
    #[allow(unused)]
    profile: ProfileOpt,
}

impl Compose {
    pub fn run(&self, _client_config: StudioClientConfig) -> RoverResult<RoverOutput> {
        let mut err = RoverError::new(anyhow!("This command has been deprecated."));
        let suggestion = match &self.supergraph_yaml {
            FileDescriptorType::Stdin => {
                "Please set `federation_version: 2` in the configuration you passed via stdin, and run `rover supergraph compose`".to_string()
            },
            FileDescriptorType::File(config_path) => {
                format!("Please set `federation_version: 2` in `{}` and run `rover supergraph compose`", &config_path)
            }
        };
        err.set_suggestion(RoverErrorSuggestion::Adhoc(suggestion));
        Err(err)
    }
}

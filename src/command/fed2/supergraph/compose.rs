use crate::Suggestion;
use crate::{anyhow, command::RoverOutput, error::RoverError, Result};
use crate::{
    options::ProfileOpt,
    utils::{client::StudioClientConfig, parsers::FileDescriptorType},
};

use saucer::{clap, Parser};
use serde::Serialize;

#[derive(Debug, Serialize, Parser)]
pub struct Compose {
    /// The relative path to the supergraph configuration file. You can pass `-` to use stdin instead of a file.
    #[clap(long = "config")]
    #[serde(skip_serializing)]
    supergraph_yaml: FileDescriptorType,

    #[clap(flatten)]
    #[allow(unused)]
    profile: ProfileOpt,
}

impl Compose {
    pub fn run(&self, _client_config: StudioClientConfig) -> Result<RoverOutput> {
        let mut err = RoverError::new(anyhow!("This command has been deprecated."));
        let suggestion = match &self.supergraph_yaml {
            FileDescriptorType::Stdin => {
                "Please set `federation_version: 2` in the configuration you passed via stdin, and run `rover supergraph compose`".to_string()
            },
            FileDescriptorType::File(config_path) => {
                format!("Please set `federation_version: 2` in `{}` and run `rover supergraph compose`", &config_path)
            }
        };
        err.set_suggestion(Suggestion::Adhoc(suggestion));
        Err(err)
    }
}

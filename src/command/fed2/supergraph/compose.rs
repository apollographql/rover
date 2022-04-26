use crate::utils::{
    client::StudioClientConfig,
    parsers::{parse_file_descriptor, FileDescriptorType},
};
use crate::Suggestion;
use crate::{anyhow, command::RoverOutput, error::RoverError, Result};

use serde::Serialize;
use structopt::StructOpt;

#[derive(Debug, Serialize, StructOpt)]
pub struct Compose {
    /// The relative path to the supergraph configuration file. You can pass `-` to use stdin instead of a file.
    #[structopt(long = "config", parse(try_from_str = parse_file_descriptor))]
    #[serde(skip_serializing)]
    supergraph_yaml: FileDescriptorType,

    /// Name of configuration profile to use
    #[structopt(long = "profile", default_value = "default")]
    #[serde(skip_serializing)]
    _profile_name: String,
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

use clap::Parser;
use serde::Serialize;
use serde_json::json;

use crate::{
    RoverOutput, RoverResult,
    command::CliOutput,
    utils::{expansion::expand, parsers::FileDescriptorType},
};

#[derive(Debug, Serialize, Parser)]
pub struct Expand {
    /// The relative path to the supergraph configuration file. You can pass `-` to use stdin instead of a file.
    #[arg(long = "config")]
    supergraph_yaml: FileDescriptorType,
}

impl Expand {
    pub fn run(&self) -> RoverResult<RoverOutput> {
        let contents = self
            .supergraph_yaml
            .read_file_descriptor("supergraph config", &mut std::io::stdin())?;
        let expanded = expand(serde_yaml::from_str(&contents)?)?;
        let expanded_config = serde_yaml::to_string(&expanded)?;

        Ok(RoverOutput::CliOutput(Box::new(ExpandOutput {
            expanded_config,
        })))
    }
}

/// Output for `rover supergraph config expand`: the supergraph configuration file rendered as
/// YAML after all variable references (e.g. `${env.X}`) have been expanded.
#[derive(Debug)]
pub struct ExpandOutput {
    pub expanded_config: String,
}

impl CliOutput for ExpandOutput {
    fn text(&self) -> String {
        self.expanded_config.clone()
    }

    fn json(&self) -> Result<serde_json::Value, serde_json::Error> {
        Ok(json!({ "expanded_config": self.expanded_config }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_is_the_expanded_config_verbatim() {
        let output = ExpandOutput {
            expanded_config: "federation_version: =2.9.0\n".to_string(),
        };
        assert_eq!(output.text(), "federation_version: =2.9.0\n");
    }

    #[test]
    fn json_wraps_the_expanded_config_in_a_field() {
        let output = ExpandOutput {
            expanded_config: "federation_version: =2.9.0\n".to_string(),
        };
        assert_eq!(
            output.json().unwrap(),
            json!({ "expanded_config": "federation_version: =2.9.0\n" })
        );
    }
}

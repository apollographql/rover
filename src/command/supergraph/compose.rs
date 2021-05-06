use crate::{anyhow, command::RoverStdout, Result};

use crate::utils::client::StudioClientConfig;
use ansi_term::Colour::Red;
use camino::Utf8PathBuf;
use serde::Serialize;
use structopt::StructOpt;

use super::config;

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
    pub fn run(&self, client_config: StudioClientConfig) -> Result<RoverStdout> {
        let supergraph_config = config::parse_supergraph_config(&self.config_path)?;
        let subgraph_definitions = supergraph_config.get_subgraph_definitions(
            &self.config_path,
            client_config,
            &self.profile_name,
        )?;

        match harmonizer::harmonize(subgraph_definitions) {
            Ok(core_schema) => Ok(RoverStdout::CoreSchema(core_schema)),
            Err(composition_errors) => {
                let num_failures = composition_errors.len();
                for composition_error in composition_errors {
                    eprintln!("{} {}", Red.bold().paint("error:"), &composition_error)
                }
                match num_failures {
                    0 => unreachable!("Composition somehow failed with no composition errors."),
                    1 => Err(
                        anyhow!("Encountered 1 composition error while composing the graph.")
                            .into(),
                    ),
                    _ => Err(anyhow!(
                        "Encountered {} composition errors while composing the graph.",
                        num_failures
                    )
                    .into()),
                }
            }
        }
    }
}

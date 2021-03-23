use crate::{anyhow, command::RoverStdout, Result};

use ansi_term::Colour::Red;
use camino::Utf8PathBuf;
use serde::Serialize;
use structopt::StructOpt;

use super::config;

#[derive(Debug, Serialize, StructOpt)]
pub struct Build {
    /// The relative path to the core configuration file.
    #[structopt(long = "config")]
    #[serde(skip_serializing)]
    config_path: Utf8PathBuf,
}

impl Build {
    pub fn run(&self) -> Result<RoverStdout> {
        let core_config = config::parse_core_config(&self.config_path)?;
        let subgraph_definitions = core_config.get_subgraph_definitions(&self.config_path)?;

        match harmonizer::harmonize(subgraph_definitions) {
            Ok(csdl) => Ok(RoverStdout::CSDL(csdl)),
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

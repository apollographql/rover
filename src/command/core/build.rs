use crate::{anyhow, command::RoverStdout, Result};

use ansi_term::Colour::Red;
use camino::Utf8PathBuf;
use harmonizer::{self, ServiceDefinition as SubgraphDefinition};
use serde::{Deserialize, Serialize};
use structopt::StructOpt;

use std::collections::HashMap;
use std::fs;

#[derive(Debug, Serialize, StructOpt)]
pub struct Build {
    /// The relative path to the core configuration file.
    #[structopt(long = "config")]
    #[serde(skip_serializing)]
    core_config: Utf8PathBuf,
}

#[derive(Deserialize)]
struct CoreConfig {
    subgraphs: HashMap<String, Subgraph>,
}

#[derive(Deserialize)]
struct Subgraph {
    routing_url: String,
    path: Utf8PathBuf,
}

impl Build {
    pub fn run(&self) -> Result<RoverStdout> {
        let raw_core_config = fs::read_to_string(&self.core_config)?;
        let parsed_core_config: CoreConfig = serde_yaml::from_str(&raw_core_config)?;
        let mut subgraphs = Vec::new();
        for (subgraph_name, subgraph_data) in parsed_core_config.subgraphs {
            let relative_schema_path = if let Some(parent) = self.core_config.parent() {
                let mut schema_path = parent.to_path_buf();
                schema_path.push(subgraph_data.path);
                schema_path
            } else {
                subgraph_data.path
            };
            let schema = fs::read_to_string(&relative_schema_path)?;
            let subgraph_definition =
                SubgraphDefinition::new(subgraph_name, subgraph_data.routing_url, &schema);
            subgraphs.push(subgraph_definition);
        }

        match harmonizer::harmonize(subgraphs) {
            Ok(csdl) => Ok(RoverStdout::CSDL(csdl)),
            Err(composition_errors) => {
                let num_failures = composition_errors.len();
                for composition_error in composition_errors {
                    eprintln!("{} {}", Red.bold().paint("error:"), &composition_error)
                }
                match num_failures {
                    0 => Ok(RoverStdout::None),
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

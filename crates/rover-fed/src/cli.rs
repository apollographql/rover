use harmonizer::harmonize;

use supergraph_config::SupergraphConfig;

use camino::Utf8PathBuf;
use serde::{Deserialize, Serialize};
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(
    name = "rover-fed",
    about = "A utility for composing multiple subgraphs into a supergraph"
)]
pub struct RoverFed {
    #[structopt(subcommand)]
    command: Command,

    /// Print output as JSON.
    #[structopt(long, global = true)]
    json: bool,
}

impl RoverFed {
    pub fn run(&self) -> Result<(), anyhow::Error> {
        let output = match &self.command {
            Command::Compose(command) => command.run(),
        }?;

        if self.json {
            println!("{}", serde_json::json!(output));
        } else {
            println!("{}", output.supergraph_sdl)
        }

        Ok(())
    }
}

#[derive(Debug, StructOpt)]
enum Command {
    /// Compose a supergraph from a fully resolved supergraph config YAML
    Compose(Compose),
}

#[derive(Debug, StructOpt)]
struct Compose {
    /// The path to the fully resolved supergraph YAML.
    ///
    /// NOTE: Each subgraph entry MUST contain raw SDL
    /// as the schema source.
    config_file: Utf8PathBuf,
}

impl Compose {
    fn run(&self) -> Result<CompositionOutput, anyhow::Error> {
        let supergraph_config = SupergraphConfig::new_from_yaml_file(&self.config_file)?;
        let subgraph_definitions: harmonizer::ServiceList =
            supergraph_config.get_subgraph_definitions()?;
        let supergraph_sdl = harmonize(subgraph_definitions)?;

        Ok(CompositionOutput { supergraph_sdl })
    }
}

/// CompositionOutput contains information about the supergraph that was composed.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
struct CompositionOutput {
    /// Supergraph SDL can be used to start a gateway instance
    supergraph_sdl: String,
}

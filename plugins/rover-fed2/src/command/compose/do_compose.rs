use apollo_supergraph_config::SupergraphConfig;
use camino::Utf8PathBuf;
use harmonizer_fed_two::{harmonize, CompositionOutput};
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
pub struct Compose {
    /// The path to the fully resolved supergraph YAML.
    ///
    /// NOTE: Each subgraph entry MUST contain raw SDL
    /// as the schema source.
    config_file: Utf8PathBuf,
}

impl Compose {
    pub fn run(&self) -> Result<CompositionOutput, anyhow::Error> {
        let supergraph_config = SupergraphConfig::new_from_yaml_file(&self.config_file)?;
        let subgraph_definitions = supergraph_config.get_subgraph_definitions()?;
        Ok(harmonize(subgraph_definitions)?)
    }
}

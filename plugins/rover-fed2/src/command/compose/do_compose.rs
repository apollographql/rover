use apollo_federation_types::{build::BuildResult, config::SupergraphConfig};
use camino::Utf8PathBuf;
use harmonizer_fed_two::harmonize;
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
    pub fn run(&self) -> BuildResult {
        let supergraph_config = SupergraphConfig::new_from_yaml_file(&self.config_file)?;
        let subgraph_definitions = supergraph_config.get_subgraph_definitions()?;
        harmonize(subgraph_definitions)
    }
}

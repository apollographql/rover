use camino::Utf8PathBuf;
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
    pub fn run(&self, json: bool) -> Result<(), anyhow::Error> {
        use apollo_supergraph_config::SupergraphConfig;
        use harmonizer::harmonize;

        let supergraph_config = SupergraphConfig::new_from_yaml_file(&self.config_file)?;
        let subgraph_definitions = supergraph_config.get_subgraph_definitions()?;
        let composition_output = harmonize(subgraph_definitions)?;
        if json {
            println!("{}", serde_json::json!(composition_output));
        } else {
            println!("{}", composition_output.supergraph_sdl)
        }
        Ok(())
    }
}

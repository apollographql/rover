use std::io::prelude::*;

use apollo_federation_types::build::SubgraphDefinition;
use apollo_federation_types::config::{FederationVersion, SupergraphConfig};
use saucer::{anyhow, Utf8PathBuf};

use crate::command::supergraph::compose::Compose;
use crate::command::RoverOutput;
use crate::options::PluginOpts;
use crate::utils::client::StudioClientConfig;
use crate::{error::RoverError, Result};

#[derive(Debug, Clone)]
pub struct ComposeRunner {
    compose: Compose,
    override_install_path: Option<Utf8PathBuf>,
    client_config: StudioClientConfig,
    subgraph_definitions: Vec<SubgraphDefinition>,
    write_path: Utf8PathBuf,
}

impl ComposeRunner {
    pub fn new(
        compose_opts: PluginOpts,
        override_install_path: Option<Utf8PathBuf>,
        client_config: StudioClientConfig,
        subgraph_definitions: Vec<SubgraphDefinition>,
        write_path: Utf8PathBuf,
    ) -> Self {
        Self {
            compose: Compose::new(compose_opts),
            override_install_path,
            client_config,
            subgraph_definitions,
            write_path,
        }
    }

    pub fn add_subgraph(&mut self, subgraph_definition: SubgraphDefinition) -> Result<()> {
        self.subgraph_definitions.push(subgraph_definition);
        self.run()
    }

    pub fn run(&self) -> Result<()> {
        let mut supergraph_config = SupergraphConfig::from(self.subgraph_definitions.clone());
        supergraph_config.set_federation_version(FederationVersion::LatestFedTwo);
        match self.compose.compose(
            self.override_install_path.clone(),
            self.client_config.clone(),
            &mut supergraph_config.clone(),
        ) {
            Ok(build_result) => match &build_result {
                RoverOutput::CompositionResult {
                    supergraph_sdl,
                    hints: _,
                    federation_version: _,
                } => {
                    let context = format!("could not write SDL to {}", &self.write_path);
                    match std::fs::File::create(&self.write_path) {
                        Ok(mut opened_file) => {
                            if let Err(e) = opened_file.write_all(supergraph_sdl.as_bytes()) {
                                Err(RoverError::new(
                                    anyhow!("{}", e)
                                        .context("could not write bytes")
                                        .context(context),
                                ))
                            } else if let Err(e) = opened_file.flush() {
                                Err(RoverError::new(
                                    anyhow!("{}", e)
                                        .context("could not flush file")
                                        .context(context),
                                ))
                            } else {
                                eprintln!(
                                    "wrote updated supergraph schema to {}",
                                    &self.write_path
                                );
                                Ok(())
                            }
                        }
                        Err(e) => Err(RoverError::new(anyhow!("{}", e).context(context))),
                    }
                }
                _ => unreachable!(),
            },
            Err(e) => Err(anyhow!("{}", e).into()),
        }
    }
}

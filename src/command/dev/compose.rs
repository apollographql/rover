use std::io::prelude::*;

use saucer::{anyhow, Utf8PathBuf};

use crate::command::dev::socket::DevRunner;
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
    write_path: Utf8PathBuf,
}

impl ComposeRunner {
    pub fn new(
        compose_opts: PluginOpts,
        override_install_path: Option<Utf8PathBuf>,
        client_config: StudioClientConfig,
        write_path: Utf8PathBuf,
    ) -> Self {
        Self {
            compose: Compose::new(compose_opts),
            override_install_path,
            client_config,
            write_path,
        }
    }

    pub fn run(&self, composer_state: &DevRunner) -> Result<()> {
        let mut supergraph_config = composer_state.supergraph_config();
        match self.compose.compose(
            self.override_install_path.clone(),
            self.client_config.clone(),
            &mut supergraph_config,
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
                                tracing::info!(
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

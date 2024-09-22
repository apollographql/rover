use std::fs;
use std::io::prelude::*;

use anyhow::{Context, Error};
use apollo_federation_types::config::{FederationVersion, SupergraphConfig};
use camino::Utf8PathBuf;
use rover_std::{errln, Fs};

use crate::command::dev::do_dev::log_err_and_continue;
use crate::command::supergraph::compose::{Compose, CompositionOutput};
use crate::options::PluginOpts;
use crate::utils::client::StudioClientConfig;
use crate::{RoverError, RoverResult};

#[derive(Debug)]
pub(crate) struct ComposeRunner {
    compose: Compose,
    override_install_path: Option<Utf8PathBuf>,
    client_config: StudioClientConfig,
    write_path: Utf8PathBuf,
    composition_state: Option<RoverResult<CompositionOutput>>,
}

impl ComposeRunner {
    pub(crate) async fn new(
        compose_opts: PluginOpts,
        override_install_path: Option<Utf8PathBuf>,
        client_config: StudioClientConfig,
        write_path: Utf8PathBuf,
        federation_version: FederationVersion,
    ) -> RoverResult<Self> {
        let compose = Compose::new(compose_opts);
        // TODO: compose immediately on startup, which means this pre-emptive plugin check is unnecessary
        compose
            .maybe_install_supergraph(
                override_install_path.clone(),
                client_config.clone(),
                federation_version,
            )
            .await?;
        Ok(Self {
            compose,
            override_install_path,
            client_config,
            write_path,
            composition_state: None,
        })
    }

    /// TODO: extract router-focused state handling somewhere else, so this can be re-used by lsp
    pub async fn run(
        &mut self,
        supergraph_config: &mut SupergraphConfig,
    ) -> Result<Option<CompositionOutput>, String> {
        let prev_state = self.composition_state();
        self.composition_state = Some(
            self.compose
                .exec(
                    self.override_install_path.clone(),
                    self.client_config.clone(),
                    supergraph_config,
                    None,
                )
                .await,
        );
        let new_state = self.composition_state();

        match (prev_state, new_state) {
            // wasn't composed, now composed
            (None, Some(Ok(new_success))) | (Some(Err(_)), Some(Ok(new_success))) => {
                let _ = self
                    .update_supergraph_schema(&new_success.supergraph_sdl)
                    .map_err(log_err_and_continue);
                Ok(Some(new_success))
            }
            // had a composition error, now a new composition error
            (Some(Err(prev_err)), Some(Err(new_err))) => {
                if prev_err != new_err {
                    let _ = self.remove_supergraph_schema();
                }
                Err(new_err)
            }
            // had a successful composition, now a new successful composition
            (Some(Ok(prev_success)), Some(Ok(new_success))) => {
                if prev_success != new_success {
                    let _ = self.update_supergraph_schema(&new_success.supergraph_sdl);
                    Ok(Some(new_success))
                } else {
                    Ok(None)
                }
            }
            // not composed (this should be unreachable in practice)
            (_, None) => {
                let _ = self.remove_supergraph_schema();
                Ok(None)
            }
            // now has an error
            (_, Some(Err(new_err))) => {
                let _ = self.remove_supergraph_schema();
                Err(new_err)
            }
        }
    }

    fn remove_supergraph_schema(&self) -> RoverResult<()> {
        if Fs::assert_path_exists(&self.write_path).is_ok() {
            errln!("composition failed, killing the router");
            Ok(fs::remove_file(&self.write_path)
                .with_context(|| format!("could not remove {}", &self.write_path))?)
        } else {
            Ok(())
        }
    }

    fn update_supergraph_schema(&self, sdl: &str) -> RoverResult<()> {
        tracing::info!("composition succeeded, updating the supergraph schema...");
        let context = format!("could not write SDL to {}", &self.write_path);
        match fs::File::create(&self.write_path) {
            Ok(mut opened_file) => {
                if let Err(e) = opened_file.write_all(sdl.as_bytes()) {
                    Err(RoverError::new(
                        Error::new(e)
                            .context("could not write bytes")
                            .context(context),
                    ))
                } else if let Err(e) = opened_file.flush() {
                    Err(RoverError::new(
                        Error::new(e)
                            .context("could not flush file")
                            .context(context),
                    ))
                } else {
                    tracing::info!("wrote updated supergraph schema to {}", &self.write_path);
                    Ok(())
                }
            }
            Err(e) => Err(RoverError::new(Error::new(e).context(context))),
        }
    }

    pub fn composition_state(&self) -> Option<Result<CompositionOutput, String>> {
        self.composition_state.as_ref().map(|s| match s {
            Ok(comp) => Ok(comp.clone()),
            Err(err) => Err(err.to_string()),
        })
    }
}

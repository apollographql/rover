use std::fs;
use std::io::prelude::*;

use anyhow::{Context, Error};
use apollo_federation_types::rover::BuildResult;
use camino::Utf8PathBuf;
use rover_client::RoverClientError;
use rover_std::{errln, Fs};

use crate::command::dev::do_dev::log_err_and_continue;
use crate::federation::Composer;
use crate::{RoverError, RoverResult};

#[derive(Debug)]
pub(crate) struct ComposeRunner {
    pub(crate) composer: Composer,
    write_path: Utf8PathBuf,
    composition_state: Option<BuildResult>, // TODO: this doesn't need to be an option because we compose on startup now
}

impl ComposeRunner {
    pub(crate) async fn new(composer: Composer, write_path: Utf8PathBuf) -> RoverResult<Self> {
        Ok(Self {
            composer,
            write_path,
            composition_state: None,
        })
    }

    /// TODO: extract router-focused state handling somewhere else, so this can be re-used by lsp
    /// TODO: Why do we return successful output here?
    pub async fn run(&mut self) -> RoverResult<()> {
        let prev_state = self.composition_state.take();
        let new_state = self.composer.compose(None).await?;

        let output = match (prev_state, &new_state) {
            // wasn't composed, now composed
            (None, Ok(new_success)) | (Some(Err(_)), Ok(new_success)) => {
                let _ = self
                    .update_supergraph_schema(&new_success.supergraph_sdl)
                    .map_err(log_err_and_continue);
                Ok(())
            }
            // had a composition error, now a new composition error
            (Some(Err(prev_err)), Err(new_err)) => {
                if prev_err != *new_err {
                    let _ = self.remove_supergraph_schema();
                }
                Err(RoverError::from(RoverClientError::BuildErrors {
                    source: new_err.clone(),
                    num_subgraphs: self.composer.supergraph_config.subgraphs.len(),
                }))
            }
            // had a successful composition, now a new successful composition
            (Some(Ok(prev_success)), Ok(new_success)) => {
                if prev_success != *new_success {
                    let _ = self.update_supergraph_schema(&new_success.supergraph_sdl);
                    Ok(())
                } else {
                    Ok(()) // TODO: don't restart the router up a level, so this will be no-op
                }
            }
            // now has an error
            (_, Err(new_err)) => {
                let _ = self.remove_supergraph_schema();
                Err(RoverError::from(RoverClientError::BuildErrors {
                    source: new_err.clone(),
                    num_subgraphs: self.composer.supergraph_config.subgraphs.len(),
                }))
            }
        };
        self.composition_state = Some(new_state);
        output
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
}

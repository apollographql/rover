use anyhow::anyhow;
use apollo_federation_types::config::{SchemaSource, SupergraphConfig};

use crate::{
    command::dev::SupergraphOpts,
    options::{PluginOpts, ProfileOpt},
    utils::{client::StudioClientConfig, supergraph_config::get_supergraph_config},
    RoverError, RoverResult,
};

pub struct Runner {
    client_config: StudioClientConfig,
    supergraph_opts: SupergraphOpts,
}

impl Runner {
    pub fn new(client_config: &StudioClientConfig, supergraph_opts: &SupergraphOpts) -> Self {
        Self {
            client_config: client_config.clone(),
            supergraph_opts: supergraph_opts.clone(),
        }
    }

    pub async fn run(&mut self, profile: &ProfileOpt) -> RoverResult<()> {
        tracing::info!("initializing main `rover dev process`");

        let _supergraph_config = self.load_supergraph_config(profile).await?;

        Ok(())
    }

    async fn load_supergraph_config(&self, profile: &ProfileOpt) -> RoverResult<SupergraphConfig> {
        get_supergraph_config(
            &self.supergraph_opts.graph_ref,
            &self.supergraph_opts.supergraph_config_path,
            self.supergraph_opts.federation_version.as_ref(),
            self.client_config.clone(),
            profile,
            false,
        )
        .await
        .map_err(|_| RoverError::new(anyhow!("TODO: get actual error")))?
        .ok_or_else(|| RoverError::new(anyhow!("Why is supergraph config None?")))
    }
}

use anyhow::anyhow;
use apollo_federation_types::config::SupergraphConfig;
use futures::stream::StreamExt;

use crate::{
    command::dev::{
        subtask::{Subtask, SubtaskRunUnit},
        watcher::{file::FileWatcher, supergraph_config::SupergraphConfigWatcher},
        SupergraphOpts,
    },
    options::ProfileOpt,
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
        let supergraph_config = self.load_supergraph_config(profile).await?;

        // Start supergraph watcher.
        self.start_supergraph_config_watcher(supergraph_config.clone())
            .await;

        Ok(())
    }

    async fn start_supergraph_config_watcher(&self, supergraph_config: SupergraphConfig) {
        let f = FileWatcher::new(
            self.supergraph_opts
                .supergraph_config_path
                .as_ref()
                .unwrap()
                .to_path_buf()
                .unwrap()
                .clone(),
        );
        let supergraph_config_watcher = SupergraphConfigWatcher::new(f, supergraph_config);

        let (mut supergraph_stream, supergraph_subtask) = Subtask::new(supergraph_config_watcher);
        supergraph_subtask.run();

        tokio::task::spawn(async move {
            loop {
                supergraph_stream.next().await;
                eprintln!("supergraph update");
            }
        })
        .await
        .unwrap();
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

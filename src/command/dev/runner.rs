use anyhow::anyhow;
use apollo_federation_types::config::{SchemaSource, SupergraphConfig};
use futures::stream::StreamExt;
use tokio::task::JoinHandle;

use crate::{
    command::dev::{
        subtask::{Subtask, SubtaskRunUnit},
        watcher::{
            file::FileWatcher,
            subgraph_config::{SubgraphConfigWatcher, SubgraphConfigWatcherKind},
            supergraph_config::SupergraphConfigWatcher,
        },
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

        // Start supergraph and subgraph watchers.
        let handles = self.start_config_watchers(supergraph_config.clone());

        futures::future::join_all(handles).await;

        Ok(())
    }

    fn start_config_watchers(&self, supergraph_config: SupergraphConfig) -> Vec<JoinHandle<()>> {
        let mut futs = vec![];

        // Create a new supergraph config file watcher.
        let f = FileWatcher::new(
            self.supergraph_opts
                .supergraph_config_path
                .as_ref()
                .unwrap()
                .to_path_buf()
                .unwrap()
                .clone(),
        );
        let watcher = SupergraphConfigWatcher::new(f, supergraph_config.clone());

        // Create and run the file watcher in a sub task.
        let (mut supergraph_stream, supergraph_subtask) = Subtask::new(watcher);
        supergraph_subtask.run();

        futs.push(tokio::task::spawn(async move {
            while let Some(_) = supergraph_stream.next().await {
                eprintln!("supergraph update");
            }
        }));

        // Create subgraph config watchers.
        for (subgraph, subgraph_config) in supergraph_config.into_iter() {
            match subgraph_config.schema {
                SchemaSource::File { ref file } => {
                    // Create a new file watcher kind.
                    let kind = SubgraphConfigWatcherKind::File(FileWatcher::new(file.clone()));
                    // Construct a subgraph config watcher from the file watcher kind.
                    let watcher = SubgraphConfigWatcher::new(kind, subgraph_config);
                    // Create and run the file watcher in a sub task.
                    let (mut stream, subtask) = Subtask::new(watcher);
                    subtask.run();

                    let task = tokio::task::spawn(async move {
                        while let Some(_) = stream.next().await {
                            eprintln!("subgraph update: {subgraph}");
                        }
                    });
                    futs.push(task);
                }
                SchemaSource::SubgraphIntrospection { .. } => todo!(),
                SchemaSource::Sdl { .. } => todo!(),
                SchemaSource::Subgraph { .. } => todo!(),
            };
        }

        futs
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

use apollo_federation_types::config::SupergraphConfig;
use futures::stream::StreamExt;
use rover_std::errln;
use tokio::task::JoinHandle;

use crate::{
    composition::watchers::{
        subtask::{Subtask, SubtaskRunUnit},
        watcher::{
            file::FileWatcher,
            subgraph_config::{SubgraphConfigWatcher, SubgraphConfigWatcherKind},
            supergraph_config::SupergraphConfigWatcher,
        },
    },
    RoverResult,
};

use super::supergraph::config::FinalSupergraphConfig;

// TODO: handle retry flag for subgraphs (see rover dev help)
pub struct Runner {
    supergraph_config: FinalSupergraphConfig,
}

impl Runner {
    pub fn new(final_supergraph_config: FinalSupergraphConfig) -> Self {
        Self {
            supergraph_config: final_supergraph_config,
        }
    }

    pub async fn run(&self) -> RoverResult<()> {
        // Start supergraph and subgraph watchers.
        let handles = self.start_config_watchers();

        futures::future::join_all(handles).await;

        Ok(())
    }

    fn start_config_watchers(&self) -> Vec<JoinHandle<()>> {
        let supergraph_config: SupergraphConfig = self.supergraph_config.clone().into();
        let mut futs = vec![];

        // Create a new supergraph config file watcher.
        let f = FileWatcher::new(self.supergraph_config.path().clone());
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
            // Create a new watcher kind.
            let watcher_kind: SubgraphConfigWatcherKind = match subgraph_config.schema.try_into() {
                Ok(kind) => kind,
                Err(err) => {
                    errln!("skipping subgraph {subgraph}: {err}");
                    continue;
                }
            };

            // Construct a subgraph config watcher from the file watcher kind.
            let watcher = SubgraphConfigWatcher::new(watcher_kind, &subgraph);
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

        futs
    }
}

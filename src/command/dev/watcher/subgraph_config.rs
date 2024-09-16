use std::{marker::Send, pin::Pin};

use apollo_federation_types::config::SubgraphConfig;
use futures::{Stream, StreamExt};
use rover_std::errln;
use tap::TapFallible;
use tokio::{sync::mpsc::UnboundedSender, task::AbortHandle};

use crate::command::dev::{introspect::IntrospectRunnerKind, subtask::SubtaskHandleUnit};

use super::file::FileWatcher;

#[derive(Debug, Clone)]
pub enum SubgraphConfigWatcherKind {
    /// Watch a file on disk.
    File(FileWatcher),
    /// Poll an endpoint via introspection.
    _Introspect(IntrospectRunnerKind, u64),
    /// Don't ever update, schema is only pulled once.
    _Once(String),
}

impl SubgraphConfigWatcherKind {
    async fn watch(&self) -> Pin<Box<dyn Stream<Item = String> + Send>> {
        match self {
            Self::File(file_watcher) => file_watcher.clone().watch(),
            Self::_Introspect(_, _) => todo!(),
            Self::_Once(_) => todo!(),
        }
    }
}

pub struct SubgraphConfigWatcher {
    watcher: SubgraphConfigWatcherKind,
    //subgraph_config: SubgraphConfig,
}

impl SubgraphConfigWatcher {
    pub fn new(watcher: SubgraphConfigWatcherKind, _subgraph_config: SubgraphConfig) -> Self {
        Self {
            watcher,
            //subgraph_config,
        }
    }
}

/// A unit struct denoting a change to a subgraph, used by composition to know whether to recompose
pub struct SubgraphChanged;

impl SubtaskHandleUnit for SubgraphConfigWatcher {
    type Output = SubgraphChanged;

    fn handle(self, sender: UnboundedSender<Self::Output>) -> AbortHandle {
        tokio::spawn(async move {
            while let Some(content) = self.watcher.watch().await.next().await {
                let parsed_config: Result<SubgraphConfig, serde_yaml::Error> =
                    serde_yaml::from_str(&content);

                // We're only looking at whether a subgraph has changed, but we won't emit events
                // if the subgraph config can't be parsed to fail early for composition
                match parsed_config {
                    Ok(_subgraph_config) => {
                        let _ = sender
                            .send(SubgraphChanged)
                            .tap_err(|err| tracing::error!("{:?}", err));
                    }
                    Err(err) => {
                        tracing::error!("Could not parse subgraph config file: {:?}", err);
                        errln!("could not parse subgraph config file");
                    }
                }
            }
        })
        .abort_handle()
    }
}

use std::{marker::Send, pin::Pin};

use apollo_federation_types::config::SubgraphConfig;
use futures::{Stream, StreamExt};
use tokio::{sync::mpsc::UnboundedSender, task::AbortHandle};

use crate::command::dev::{
    introspect::{IntrospectRunnerKind, UnknownIntrospectRunner},
    subtask::SubtaskHandleUnit,
};

use super::file::FileWatcher;

#[derive(Debug, Clone)]
pub enum SubgraphConfigWatcherKind {
    /// Watch a file on disk.
    File(FileWatcher),
    /// Poll an endpoint via introspection.
    Introspect(IntrospectRunnerKind, u64),
    /// Don't ever update, schema is only pulled once.
    Once(String),
}

impl SubgraphConfigWatcherKind {
    async fn watch(&self) -> Pin<Box<dyn Stream<Item = String> + Send>> {
        match self {
            Self::File(file_watcher) => file_watcher.clone().watch(),
            Self::Introspect(_, _) => todo!(),
            Self::Once(_) => todo!(),
        }
    }
}

pub struct SubgraphConfigWatcher {
    watcher: SubgraphConfigWatcherKind,
    subgraph_config: SubgraphConfig,
}

impl SubgraphConfigWatcher {
    pub fn new(watcher: SubgraphConfigWatcherKind, subgraph_config: SubgraphConfig) -> Self {
        Self {
            watcher,
            subgraph_config,
        }
    }
}

pub struct SubgraphChanged;

impl SubtaskHandleUnit for SubgraphConfigWatcher {
    type Output = SubgraphChanged;

    // nb: since we're just sending, we don't need to return anything other than the abort handle
    // so that we can eventually kill it if needed (ie, we're not joining the task, we're just
    // running it in the background, to explain why this signature might look weird; cf the
    // supergraph_config watcher)
    fn handle(self, sender: UnboundedSender<Self::Output>) -> AbortHandle {
        tokio::spawn(async move {
            let mut latest_subgraph_config = self.subgraph_config.clone();
            // also ugly
            while let Some(content) = self.watcher.watch().await.next().await {
                // 1) somehow get the subgraphconfig from the string; I don't see an constructor or
                //    anything, but maybe the struct can be used directly
                // 2) if it converts okay, compare it against self.subgraph_config; otherwise,
                //    handle the error in some way (not sure what the best approach would be;
                //    supergraph_config looks like it just traces it and prints it)
                //
                // (2) makes me think that the SomeOutput should really be SubgraphConfigDiff, very
                // similar to the SupergraphConfigDiff; we can then, based on whether there's a
                // diff, emit something with the sender (similar to how the supergraph config
                // watcher works)
            }
            todo!()
        })
        .abort_handle()
    }
}

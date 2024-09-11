use std::pin::Pin;

use apollo_federation_types::config::SubgraphConfig;
use futures::StreamExt;

use crate::command::dev::dev_2::subtask::SubtaskHandleUnit;

use super::file::FileWatcher;

// figure out what should go in here; some kind of watcher, subgraph_config of some sort?
pub struct SubgraphConfigWatcher {
    watcher: SubgraphWatcher,
    subgraph_config: SubgraphConfig,
}

// I'm not really sure how this works; I know we watch by introspection and (I _think_) by
// file--so, this might be a way to capture both?
pub enum SubgraphWatcher {
    File(FileWatcher),
    // no idea what to put here, but something good; leaving commented out for now because we
    // probably need to implement the introspection watcher (pulling the bits from the old way of
    // doing it)
    //Introspection,
}

impl SubgraphWatcher {
    async fn watch(
        &self,
    ) -> Pin<Box<dyn futures::Stream<Item = std::string::String> + std::marker::Send>> {
        match self {
            Self::File(file_watcher) => file_watcher.clone().watch(),
        }
    }
}

impl SubgraphConfigWatcher {
    // this probably needs to take in some kind of enum like the above to wdetermine whether we're watching by
    // file or by introspection
    //
    // I _think_ these are the right args? this might be all we need for the constructor?
    fn _new(watcher: SubgraphWatcher, subgraph_config: SubgraphConfig) -> Self {
        Self {
            watcher,
            subgraph_config,
        }
    }
}

// I'm not really sure what this should return? maybe something similar to the
// supergraphconfigdiff? maybe use the Getters attr?
pub struct SomeOutput {}

impl SubtaskHandleUnit for SubgraphConfigWatcher {
    type Output = SomeOutput;

    // nb: since we're just sending, we don't need to return anything other than the abort handle
    // so that we can eventually kill it if needed (ie, we're not joining the task, we're just
    // running it in the background, to explain why this signature might look weird; cf the
    // supergraph_config watcher)
    fn handle(
        self,
        sender: tokio::sync::mpsc::UnboundedSender<Self::Output>,
    ) -> tokio::task::AbortHandle {
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

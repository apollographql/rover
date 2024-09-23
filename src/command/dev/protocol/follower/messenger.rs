use std::fmt::Debug;

use apollo_federation_types::javascript::SubgraphDefinition;
use crossbeam_channel::Sender;

use crate::command::dev::protocol::SubgraphUpdated;
use crate::federation::supergraph_config::ResolvedSubgraphConfig;
use crate::RoverResult;

/// Each `SubgraphWatcher` has one of these that they can use to communicate with `Orchestrator`
#[derive(Clone, Debug)]
pub(crate) struct SubgraphWatcherMessenger {
    pub(crate) sender: Sender<SubgraphUpdated>,
}

impl SubgraphWatcherMessenger {
    /// Update a subgraph in the main session
    pub fn update_subgraph(&self, subgraph: SubgraphDefinition) -> RoverResult<()> {
        self.sender.send(SubgraphUpdated {
            subgraph_name: subgraph.name,
            subgraph_config: ResolvedSubgraphConfig::new(subgraph.url, subgraph.sdl),
        })?;
        Ok(())
    }
}

use std::fmt::Debug;

use apollo_federation_types::javascript::SubgraphDefinition;
use crossbeam_channel::Sender;

use crate::command::dev::protocol::{SubgraphMessage, SubgraphName};
use crate::RoverResult;

/// Each `SubgraphWatcher` has one of these that they can use to communicate with `Orchestrator`
#[derive(Clone, Debug)]
pub(crate) struct SubgraphWatcherMessenger {
    pub(crate) sender: Sender<SubgraphMessage>,
}

impl SubgraphWatcherMessenger {
    /// Add a subgraph to the main session
    pub fn add_subgraph(&self, subgraph: &SubgraphDefinition) -> RoverResult<()> {
        self.message_orchestrator(SubgraphMessage::add(subgraph)?)?;
        Ok(())
    }

    /// Update a subgraph in the main session
    pub fn update_subgraph(&self, subgraph: &SubgraphDefinition) -> RoverResult<()> {
        self.message_orchestrator(SubgraphMessage::update(subgraph)?)?;
        Ok(())
    }

    /// Remove a subgraph from the main session
    pub fn remove_subgraph(&self, subgraph_name: &SubgraphName) -> RoverResult<()> {
        self.message_orchestrator(SubgraphMessage::Remove {
            subgraph_name: subgraph_name.clone(),
        })?;
        Ok(())
    }

    /// Send a message to the orchestrator
    fn message_orchestrator(&self, message: SubgraphMessage) -> RoverResult<()> {
        self.sender.send(message)?;
        Ok(())
    }
}

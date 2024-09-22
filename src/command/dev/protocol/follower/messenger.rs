use std::fmt::Debug;

use anyhow::anyhow;
use apollo_federation_types::javascript::SubgraphDefinition;
use crossbeam_channel::{Receiver, Sender};

use crate::command::dev::protocol::{LeaderMessageKind, SubgraphMessage, SubgraphName};
use crate::{RoverError, RoverResult};

/// Each `SubgraphWatcher` has one of these that they can use to communicate with `Orchestrator`
// TODO: Make this communicate with the a SupergraphWatcher, which then communicates with Dev's Orchestrator
#[derive(Clone, Debug)]
pub(crate) struct SubgraphWatcherMessenger {
    pub(crate) sender: Sender<SubgraphMessage>,
    pub(crate) receiver: Receiver<LeaderMessageKind>,
}

impl SubgraphWatcherMessenger {
    /// Add a subgraph to the main session
    pub fn add_subgraph(&self, subgraph: &SubgraphDefinition) -> RoverResult<()> {
        self.message_orchestrator(SubgraphMessage::add_subgraph(subgraph)?)?;
        Ok(())
    }

    /// Update a subgraph in the main session
    pub fn update_subgraph(&self, subgraph: &SubgraphDefinition) -> RoverResult<()> {
        self.message_orchestrator(SubgraphMessage::update_subgraph(subgraph)?)?;
        Ok(())
    }

    /// Remove a subgraph from the main session
    pub fn remove_subgraph(&self, subgraph_name: &SubgraphName) -> RoverResult<()> {
        self.message_orchestrator(SubgraphMessage::RemoveSubgraph {
            subgraph_name: subgraph_name.clone(),
        })?;
        Ok(())
    }

    /// Send a message to the orchestrator
    fn message_orchestrator(&self, follower_message: SubgraphMessage) -> RoverResult<()> {
        follower_message.print();
        tracing::trace!("main session sending follower message on channel");
        self.sender.send(follower_message)?;
        tracing::trace!("main session reading leader message from channel");
        let leader_message = self.receiver.recv().map_err(|e| {
            RoverError::new(anyhow!("the main process failed to update itself").context(e))
        })?;

        tracing::trace!("main session received leader message from channel");

        self.handle_leader_message(&leader_message);
        Ok(())
    }

    fn handle_leader_message(&self, leader_message: &LeaderMessageKind) {
        // TODO: Stop printing, let the orchestrator handle messages to the users
        leader_message.print();
    }
}

use std::fmt::Debug;

use anyhow::anyhow;
use apollo_federation_types::javascript::SubgraphDefinition;
use crossbeam_channel::{Receiver, Sender};

use crate::command::dev::protocol::follower::message::FollowerMessage;
use crate::command::dev::protocol::{LeaderMessageKind, SubgraphKeys, SubgraphName};
use crate::{RoverError, RoverResult};

#[derive(Clone, Debug)]
pub(crate) struct WatcherMessenger {
    pub(crate) sender: Sender<FollowerMessage>,
    pub(crate) receiver: Receiver<LeaderMessageKind>,
}

impl WatcherMessenger {
    /// Add a subgraph to the main session
    pub fn add_subgraph(&self, subgraph: &SubgraphDefinition) -> RoverResult<()> {
        self.message_leader(FollowerMessage::add_subgraph(subgraph)?)?;
        Ok(())
    }

    /// Update a subgraph in the main session
    pub fn update_subgraph(&self, subgraph: &SubgraphDefinition) -> RoverResult<()> {
        self.message_leader(FollowerMessage::update_subgraph(subgraph)?)?;
        Ok(())
    }

    /// Remove a subgraph from the main session
    pub fn remove_subgraph(&self, subgraph_name: &SubgraphName) -> RoverResult<()> {
        self.message_leader(FollowerMessage::RemoveSubgraph {
            subgraph_name: subgraph_name.clone(),
        })?;
        Ok(())
    }

    /// Send a message to the leader
    fn message_leader(
        &self,
        follower_message: FollowerMessage,
    ) -> RoverResult<Option<SubgraphKeys>> {
        follower_message.print();
        tracing::trace!("main session sending follower message on channel");
        self.sender.send(follower_message)?;
        tracing::trace!("main session reading leader message from channel");
        let leader_message = self.receiver.recv().map_err(|e| {
            RoverError::new(anyhow!("the main process failed to update itself").context(e))
        })?;

        tracing::trace!("main session received leader message from channel");

        self.handle_leader_message(&leader_message)
    }

    fn handle_leader_message(
        &self,
        leader_message: &LeaderMessageKind,
    ) -> RoverResult<Option<SubgraphKeys>> {
        leader_message.print();
        match leader_message {
            LeaderMessageKind::LeaderSessionInfo { subgraphs } => Ok(Some(subgraphs.to_vec())),
            _ => Ok(None),
        }
    }
}

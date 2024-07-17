use std::{fmt::Debug, io::BufReader, time::Duration};

use anyhow::anyhow;
use apollo_federation_types::build::SubgraphDefinition;
use crossbeam_channel::{Receiver, Sender};
use interprocess::local_socket::traits::Stream;

use crate::command::dev::protocol::{
    create_socket_name, socket_read, socket_write, FollowerMessage, LeaderMessageKind,
    SubgraphKeys, SubgraphName,
};
use crate::{RoverError, RoverErrorSuggestion, RoverResult, PKG_VERSION};

#[derive(Clone, Debug)]
pub struct FollowerMessenger {
    kind: FollowerMessengerKind,
}

impl FollowerMessenger {
    /// Create a [`FollowerMessenger`] for the main session that can talk to itself via a channel.
    pub fn from_main_session(
        follower_message_sender: Sender<FollowerMessage>,
        leader_message_receiver: Receiver<LeaderMessageKind>,
    ) -> Self {
        Self {
            kind: FollowerMessengerKind::from_main_session(
                follower_message_sender,
                leader_message_receiver,
            ),
        }
    }

    /// Create a [`FollowerMessenger`] for an attached session that can talk to the main session via a socket.
    pub fn from_attached_session(raw_socket_name: &str) -> Self {
        Self {
            kind: FollowerMessengerKind::from_attached_session(raw_socket_name.to_string()),
        }
    }

    /// Send a health check to the main session once every second to make sure it is alive.
    ///
    /// This is function will block indefinitely and should be run from a separate thread.
    pub fn health_check(&self) -> RoverResult<()> {
        loop {
            if let Err(e) =
                self.message_leader(FollowerMessage::health_check(self.is_from_main_session())?)
            {
                break Err(e);
            }
            std::thread::sleep(Duration::from_secs(1));
        }
    }

    /// Send a version check to the main session
    pub fn version_check(&self) -> RoverResult<()> {
        self.message_leader(FollowerMessage::get_version(self.is_from_main_session()))?;
        Ok(())
    }

    /// Request information about the current subgraphs in a session
    pub fn session_subgraphs(&self) -> RoverResult<Option<SubgraphKeys>> {
        self.message_leader(FollowerMessage::get_subgraphs(self.is_from_main_session()))
    }

    /// Add a subgraph to the main session
    pub fn add_subgraph(&self, subgraph: &SubgraphDefinition) -> RoverResult<()> {
        self.message_leader(FollowerMessage::add_subgraph(
            self.is_from_main_session(),
            subgraph,
        )?)?;
        Ok(())
    }

    /// Update a subgraph in the main session
    pub fn update_subgraph(&self, subgraph: &SubgraphDefinition) -> RoverResult<()> {
        self.message_leader(FollowerMessage::update_subgraph(
            self.is_from_main_session(),
            subgraph,
        )?)?;
        Ok(())
    }

    /// Remove a subgraph from the main session
    pub fn remove_subgraph(&self, subgraph: &SubgraphName) -> RoverResult<()> {
        self.message_leader(FollowerMessage::remove_subgraph(
            self.is_from_main_session(),
            subgraph,
        )?)?;
        Ok(())
    }

    /// Send a message to the leader
    fn message_leader(
        &self,
        follower_message: FollowerMessage,
    ) -> RoverResult<Option<SubgraphKeys>> {
        self.kind.message_leader(follower_message)
    }

    fn is_from_main_session(&self) -> bool {
        self.kind.is_from_main_session()
    }
}

#[derive(Clone, Debug)]
enum FollowerMessengerKind {
    FromMainSession {
        follower_message_sender: Sender<FollowerMessage>,
        leader_message_receiver: Receiver<LeaderMessageKind>,
    },
    FromAttachedSession {
        raw_socket_name: String,
    },
}

impl FollowerMessengerKind {
    fn from_main_session(
        follower_message_sender: Sender<FollowerMessage>,
        leader_message_receiver: Receiver<LeaderMessageKind>,
    ) -> Self {
        Self::FromMainSession {
            follower_message_sender,
            leader_message_receiver,
        }
    }

    fn from_attached_session(raw_socket_name: String) -> Self {
        Self::FromAttachedSession { raw_socket_name }
    }

    fn message_leader(
        &self,
        follower_message: FollowerMessage,
    ) -> RoverResult<Option<SubgraphKeys>> {
        use FollowerMessengerKind::*;
        follower_message.print();
        let leader_message = match self {
            FromMainSession {
                follower_message_sender,
                leader_message_receiver,
            } => {
                tracing::trace!("main session sending follower message on channel");
                follower_message_sender.send(follower_message)?;
                tracing::trace!("main session reading leader message from channel");
                let leader_message = leader_message_receiver.recv().map_err(|e| {
                    RoverError::new(anyhow!("the main process failed to update itself").context(e))
                });

                tracing::trace!("main session received leader message from channel");

                leader_message
            }
            FromAttachedSession { raw_socket_name } => {
                let socket_name = create_socket_name(raw_socket_name)?;
                let stream = Stream::connect(socket_name).map_err(|_| {
                    let mut err = RoverError::new(anyhow!(
                        "there is not a main `rover dev` process to report updates to"
                    ));
                    err.set_suggestion(RoverErrorSuggestion::SubmitIssue);
                    err
                })?;

                let mut stream = BufReader::new(stream);

                tracing::trace!("attached session sending follower message on socket");
                // send our message over the socket
                socket_write(&follower_message, &mut stream)?;

                tracing::trace!("attached session reading leader message from socket");
                // wait for our message to be read by the other socket handler
                // then read the response that was written back to the socket
                socket_read(&mut stream).map_err(|e| {
                    RoverError::new(
                        anyhow!(
                            "this process did not receive a message from the main process after sending {:?}",
                            &follower_message
                        )
                        .context(e),
                    )
                })
            }
        }?;

        self.handle_leader_message(&leader_message)
    }

    fn handle_leader_message(
        &self,
        leader_message: &LeaderMessageKind,
    ) -> RoverResult<Option<SubgraphKeys>> {
        leader_message.print();
        match leader_message {
            LeaderMessageKind::GetVersion {
                leader_version,
                follower_version: _,
            } => {
                self.require_same_version(leader_version)?;
                Ok(None)
            }
            LeaderMessageKind::LeaderSessionInfo { subgraphs } => Ok(Some(subgraphs.to_vec())),
            _ => Ok(None),
        }
    }

    fn require_same_version(&self, leader_version: &str) -> RoverResult<()> {
        if leader_version != PKG_VERSION {
            let mut err = RoverError::new(anyhow!(
                "The main process is running version {}, and this process is running version {}.",
                &leader_version,
                PKG_VERSION
            ));
            err.set_suggestion(RoverErrorSuggestion::Adhoc(
                "You should use the same version of `rover` to run `rover dev` sessions"
                    .to_string(),
            ));
            Err(err)
        } else {
            Ok(())
        }
    }

    fn is_from_main_session(&self) -> bool {
        matches!(
            self,
            Self::FromMainSession {
                follower_message_sender: _,
                leader_message_receiver: _
            }
        )
    }
}

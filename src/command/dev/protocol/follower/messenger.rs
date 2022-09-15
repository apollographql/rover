use crate::{error::RoverError, Result};
use crate::{Suggestion, PKG_VERSION};
use apollo_federation_types::build::SubgraphDefinition;
use interprocess::local_socket::LocalSocketStream;
use saucer::{anyhow, Context};
use std::sync::mpsc::{Receiver, SyncSender};
use std::{fmt::Debug, io::BufReader, time::Duration};

use crate::command::dev::protocol::{
    socket_read, socket_write, FollowerMessageKind, LeaderMessageKind, SubgraphKeys, SubgraphName,
};

#[derive(Debug)]
pub struct FollowerMessenger {
    kind: FollowerMessengerKind,
}

impl FollowerMessenger {
    /// Create a [`FollowerMessenger`] for the main session that can talk to itself via a channel.
    pub fn from_main_session(
        follower_message_sender: SyncSender<FollowerMessageKind>,
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
    pub fn from_attached_session(ipc_socket_addr: &str) -> Self {
        Self {
            kind: FollowerMessengerKind::from_attached_session(ipc_socket_addr.to_string()),
        }
    }

    /// Determine if this messenger sends messages to itself.
    pub fn is_main_session(&self) -> bool {
        self.kind.is_main_session()
    }

    /// Determine if this messenger sends messages to another process.
    pub fn is_attached_session(&self) -> bool {
        self.kind.is_attached_session()
    }

    /// Send a message to kill the router
    pub fn kill_router(&self) -> Result<()> {
        self.message_leader(FollowerMessageKind::kill_router())?;
        Ok(())
    }

    /// Send a health check to the main session once every second to make sure it is alive.
    ///
    /// This is function will block indefinitely and should be run from a separate thread.
    pub fn health_check(&self) -> Result<()> {
        loop {
            if let Err(e) = self.message_leader(FollowerMessageKind::health_check()) {
                break Err(e);
            }
            std::thread::sleep(Duration::from_secs(1));
        }
    }

    /// Send a version check to the main session
    pub fn version_check(&self) -> Result<()> {
        self.message_leader(FollowerMessageKind::get_version())?;
        Ok(())
    }

    /// Request information about the current subgraphs in a session
    pub fn session_subgraphs(&self) -> Result<Option<SubgraphKeys>> {
        self.message_leader(FollowerMessageKind::get_subgraphs())
    }

    /// Add a subgraph to the main session
    pub fn add_subgraph(&self, subgraph: &SubgraphDefinition) -> Result<()> {
        self.message_leader(FollowerMessageKind::add_subgraph(subgraph)?)?;
        Ok(())
    }

    /// Update a subgraph in the main session
    pub fn update_subgraph(&self, subgraph: &SubgraphDefinition) -> Result<()> {
        self.message_leader(FollowerMessageKind::update_subgraph(subgraph)?)?;
        Ok(())
    }

    /// Remove a subgraph from the main session
    pub fn remove_subgraph(&self, subgraph: &SubgraphName) -> Result<()> {
        self.message_leader(FollowerMessageKind::remove_subgraph(subgraph))?;
        Ok(())
    }

    /// Send a message to the leader
    fn message_leader(
        &self,
        follower_message: FollowerMessageKind,
    ) -> Result<Option<SubgraphKeys>> {
        self.kind.message_leader(follower_message)
    }
}

#[derive(Debug)]
enum FollowerMessengerKind {
    FromMainSession {
        follower_message_sender: SyncSender<FollowerMessageKind>,
        leader_message_receiver: Receiver<LeaderMessageKind>,
    },
    FromAttachedSession {
        ipc_socket_addr: String,
    },
}

impl FollowerMessengerKind {
    fn from_main_session(
        follower_message_sender: SyncSender<FollowerMessageKind>,
        leader_message_receiver: Receiver<LeaderMessageKind>,
    ) -> Self {
        Self::FromMainSession {
            follower_message_sender,
            leader_message_receiver,
        }
    }

    fn from_attached_session(ipc_socket_addr: String) -> Self {
        Self::FromAttachedSession { ipc_socket_addr }
    }

    fn message_leader(
        &self,
        follower_message: FollowerMessageKind,
    ) -> Result<Option<SubgraphKeys>> {
        use FollowerMessengerKind::*;
        follower_message.print(self.is_main_session());
        let leader_message = match self {
            FromMainSession {
                follower_message_sender,
                leader_message_receiver,
            } => {
                follower_message_sender.send(follower_message);
                leader_message_receiver.recv().map_err(|e| {
                    RoverError::new(
                        anyhow!("the main `rover dev` session failed to update itself").context(e),
                    )
                })
            }
            FromAttachedSession { ipc_socket_addr } => {
                let stream = LocalSocketStream::connect(&**ipc_socket_addr).map_err(|_| {
                    RoverError::new(anyhow!(
                        "the main `rover dev` session has been killed, shutting down"
                    ))
                })?;
                stream
                    .set_nonblocking(true)
                    .context("could not set socket to non-blocking mode")?;
                let mut stream = BufReader::new(stream);

                // send our message over the socket
                socket_write(&follower_message, &mut stream)?;

                // wait for our message to be read by the other socket handler
                // then read the response that was written back to the socket
                socket_read(&mut stream).map_err(|e| {
                    RoverError::new(
                        anyhow!(
                            "this `rover dev` session did not receive a message from the main `rover dev` session after sending {:?}",
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
    ) -> Result<Option<SubgraphKeys>> {
        if self.is_main_session() {
            leader_message.print();
        }
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

    fn require_same_version(&self, leader_version: &str) -> Result<()> {
        if leader_version != PKG_VERSION {
            let mut err = RoverError::new(anyhow!("The main `rover dev` session is running version {}, and this `rover dev` session is running version {}.", &leader_version, PKG_VERSION));
            err.set_suggestion(Suggestion::Adhoc(
                "You should use the same version of `rover` to run `rover dev` sessions"
                    .to_string(),
            ));
            Err(err)
        } else {
            Ok(())
        }
    }

    fn is_main_session(&self) -> bool {
        matches!(
            self,
            Self::FromMainSession {
                follower_message_sender: _,
                leader_message_receiver: _
            }
        )
    }

    fn is_attached_session(&self) -> bool {
        matches!(self, Self::FromAttachedSession { ipc_socket_addr: _ })
    }
}

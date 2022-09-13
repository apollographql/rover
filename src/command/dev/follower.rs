use crate::{error::RoverError, Result};
use crate::{Suggestion, PKG_VERSION};
use apollo_federation_types::build::SubgraphDefinition;
use interprocess::local_socket::LocalSocketStream;
use saucer::{anyhow, Context};
use serde::{Deserialize, Serialize};
use std::{fmt::Debug, io::BufReader, time::Duration};

use crate::command::dev::protocol::*;

use super::leader::LeaderMessageKind;
#[derive(Debug, Serialize, Deserialize)]
pub struct FollowerMessenger {
    ipc_socket_addr: String,
    is_main_session: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum FollowerMessageKind {
    AddSubgraph { subgraph_entry: SubgraphEntry },
    UpdateSubgraph { subgraph_entry: SubgraphEntry },
    RemoveSubgraph { subgraph_name: SubgraphName },
    KillRouter,
    GetSubgraphs,
    HealthCheck,
    GetVersion,
}

impl FollowerMessageKind {
    pub fn add_subgraph(subgraph: &SubgraphDefinition) -> Result<Self> {
        Ok(Self::AddSubgraph {
            subgraph_entry: entry_from_definition(subgraph)?,
        })
    }

    pub fn update_subgraph(subgraph: &SubgraphDefinition) -> Result<Self> {
        Ok(Self::UpdateSubgraph {
            subgraph_entry: entry_from_definition(subgraph)?,
        })
    }

    pub fn remove_subgraph(subgraph_name: &SubgraphName) -> Self {
        Self::RemoveSubgraph {
            subgraph_name: subgraph_name.to_string(),
        }
    }

    pub fn kill_router() -> Self {
        Self::KillRouter
    }

    pub fn get_subgraphs() -> Self {
        Self::GetSubgraphs
    }

    pub fn get_version() -> Self {
        Self::GetVersion
    }

    pub fn health_check() -> Self {
        Self::HealthCheck
    }

    pub fn print(&self, is_main_session: bool) {
        if is_main_session {
            tracing::debug!("sending message to self: {:?}", &self);
        } else {
            tracing::debug!("sending message to main `rover dev` session: {:?}", &self);
        }
        match &self {
            Self::AddSubgraph { subgraph_entry } => {
                if is_main_session {
                    eprintln!(
                        "starting main `rover dev` session with subgraph '{}'",
                        &subgraph_entry.0 .0
                    );
                } else {
                    eprintln!(
                        "notifying main `rover dev` session about new subgraph '{}'",
                        &subgraph_entry.0 .0
                    );
                }
            }
            Self::UpdateSubgraph { subgraph_entry } => {
                if is_main_session {
                    eprintln!(
                        "updating the schema for subgraph '{}' in this `rover dev` session",
                        &subgraph_entry.0 .0
                    );
                } else {
                    eprintln!(
                        "notifying main `rover dev` session about updated subgraph '{}'",
                        &subgraph_entry.0 .0
                    );
                }
            }
            Self::RemoveSubgraph { subgraph_name } => {
                if is_main_session {
                    eprintln!(
                        "removing subgraph '{}' from this `rover dev` session",
                        &subgraph_name
                    );
                } else {
                    eprintln!(
                        "notifying main `rover dev` session about removed subgraph '{}'",
                        &subgraph_name
                    );
                }
            }
            Self::KillRouter => {
                tracing::debug!("shutting down the router for this `rover dev` session");
            }
            Self::HealthCheck => {
                tracing::debug!("sending health check ping to the main `rover dev` session");
            }
            Self::GetVersion => {
                tracing::debug!("requesting the version of the main `rover dev` session");
            }
            Self::GetSubgraphs => {
                tracing::debug!("asking main `rover dev` session about existing subgraphs");
            }
        }
    }
}

impl FollowerMessenger {
    pub fn new(ipc_socket_addr: &str, is_main_session: bool) -> Self {
        Self {
            ipc_socket_addr: ipc_socket_addr.to_string(),
            is_main_session,
        }
    }

    pub fn new_subgraph(ipc_socket_addr: &str) -> Self {
        Self::new(ipc_socket_addr, false)
    }

    pub fn kill_router(&self) -> Result<()> {
        self.socket_message(&FollowerMessageKind::kill_router())?;
        Ok(())
    }

    pub fn health_check(&self) -> Result<()> {
        loop {
            if let Err(e) = self.socket_message(&FollowerMessageKind::health_check()) {
                break Err(e);
            }
            std::thread::sleep(Duration::from_secs(1));
        }
    }

    pub fn version_check(&self) -> Result<()> {
        self.socket_message(&FollowerMessageKind::get_version())?;
        Ok(())
    }

    pub fn session_subgraphs(&self) -> Result<Option<SubgraphKeys>> {
        self.socket_message(&FollowerMessageKind::get_subgraphs())
    }

    pub fn add_subgraph(&self, subgraph: &SubgraphDefinition) -> Result<()> {
        self.socket_message(&FollowerMessageKind::add_subgraph(subgraph)?)?;
        Ok(())
    }

    pub fn update_subgraph(&self, subgraph: &SubgraphDefinition) -> Result<()> {
        self.socket_message(&FollowerMessageKind::update_subgraph(subgraph)?)?;
        Ok(())
    }

    pub fn remove_subgraph(&self, subgraph: &SubgraphName) -> Result<()> {
        self.socket_message(&FollowerMessageKind::remove_subgraph(subgraph))?;
        Ok(())
    }

    fn should_message(&self) -> bool {
        !self.is_main_session
    }

    fn handle_leader_message(
        &self,
        leader_message: &LeaderMessageKind,
    ) -> Result<Option<SubgraphKeys>> {
        if self.should_message() {
            leader_message.print();
        }
        match leader_message {
            LeaderMessageKind::Version { version } => {
                self.require_same_version(version)?;
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

    fn socket_message(
        &self,
        follower_message: &FollowerMessageKind,
    ) -> Result<Option<SubgraphKeys>> {
        match self.connect() {
            Ok(stream) => {
                stream
                    .set_nonblocking(true)
                    .context("could not set socket to non-blocking mode")?;
                let mut stream = BufReader::new(stream);

                follower_message.print(self.is_main_session);
                // send our message over the socket
                socket_write(follower_message, &mut stream)?;

                // wait for our message to be read by the other socket handler
                // then read the response that was written back to the socket
                let result = socket_read(&mut stream);
                match result {
                    Ok(leader_message) => self.handle_leader_message(&leader_message),
                    Err(e) => {
                        tracing::info!(
                            "follower could not receive message from leader after sending {:?}",
                            &follower_message
                        );
                        Err(e)
                    }
                }
            }
            Err(e) => {
                // if we can't connect, we are not the main session
                follower_message.print(false);
                match follower_message {
                    // these two message kinds are requested on startup, if they return `None` it means
                    // that there is no current `rover dev` session to respond with
                    FollowerMessageKind::GetVersion | FollowerMessageKind::GetSubgraphs => Ok(None),
                    _ => Err(e),
                }
            }
        }
    }

    fn connect(&self) -> Result<LocalSocketStream> {
        LocalSocketStream::connect(&*self.ipc_socket_addr).map_err(|_| {
            RoverError::new(anyhow!(
                "the main `rover dev` session has been killed, shutting down"
            ))
        })
    }

    pub fn is_main_session(&self) -> bool {
        self.is_main_session
    }
}

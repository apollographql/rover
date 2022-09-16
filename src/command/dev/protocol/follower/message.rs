use crate::{anyhow, error::RoverError, utils::emoji::Emoji, Result, PKG_VERSION};
use apollo_federation_types::build::SubgraphDefinition;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

use crate::command::dev::protocol::{entry_from_definition, SubgraphEntry, SubgraphName};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FollowerMessage {
    kind: FollowerMessageKind,
    is_from_main_session: bool,
}

impl FollowerMessage {
    pub fn get_version(is_from_main_session: bool) -> Self {
        Self {
            kind: FollowerMessageKind::get_version(),
            is_from_main_session,
        }
    }

    pub fn get_subgraphs(is_from_main_session: bool) -> Self {
        Self {
            kind: FollowerMessageKind::get_subgraphs(),
            is_from_main_session,
        }
    }

    pub fn health_check(is_from_main_session: bool) -> Result<Self> {
        if is_from_main_session {
            Err(RoverError::new(anyhow!(
                "You cannot send a health check from the main `rover dev` session"
            )))
        } else {
            Ok(Self {
                kind: FollowerMessageKind::health_check(),
                is_from_main_session,
            })
        }
    }

    pub fn add_subgraph(is_from_main_session: bool, subgraph: &SubgraphDefinition) -> Result<Self> {
        Ok(Self {
            kind: FollowerMessageKind::add_subgraph(subgraph)?,
            is_from_main_session,
        })
    }

    pub fn update_subgraph(
        is_from_main_session: bool,
        subgraph: &SubgraphDefinition,
    ) -> Result<Self> {
        Ok(Self {
            kind: FollowerMessageKind::update_subgraph(subgraph)?,
            is_from_main_session,
        })
    }

    pub fn remove_subgraph(
        is_from_main_session: bool,
        subgraph_name: &SubgraphName,
    ) -> Result<Self> {
        Ok(Self {
            kind: FollowerMessageKind::remove_subgraph(subgraph_name),
            is_from_main_session,
        })
    }

    pub fn shutdown(is_from_main_session: bool) -> Self {
        Self {
            kind: FollowerMessageKind::shutdown(),
            is_from_main_session,
        }
    }

    pub fn is_from_main_session(&self) -> bool {
        self.is_from_main_session
    }

    pub fn kind(&self) -> &FollowerMessageKind {
        &self.kind
    }

    pub fn print(&self) {
        if self.is_from_main_session() {
            tracing::debug!("sending message to self: {:?}", &self);
        } else {
            tracing::debug!("sending message to main `rover dev` session: {:?}", &self);
        }
        match self.kind() {
            FollowerMessageKind::AddSubgraph { subgraph_entry } => {
                if self.is_from_main_session() {
                    eprintln!(
                        "{}starting main `rover dev` session with subgraph '{}'",
                        Emoji::Start,
                        &subgraph_entry.0 .0
                    );
                } else {
                    eprintln!(
                        "{}adding subgraph '{}' to the main `rover dev` session",
                        Emoji::New,
                        &subgraph_entry.0 .0
                    );
                }
            }
            FollowerMessageKind::UpdateSubgraph { subgraph_entry } => {
                if self.is_from_main_session() {
                    eprintln!(
                        "{}updating the schema for subgraph '{}' in this `rover dev` session",
                        Emoji::Reload,
                        &subgraph_entry.0 .0
                    );
                } else {
                    eprintln!(
                        "{}updating the schema for subgraph '{}' in the main `rover dev` session",
                        Emoji::Reload,
                        &subgraph_entry.0 .0
                    );
                }
            }
            FollowerMessageKind::RemoveSubgraph { subgraph_name } => {
                if self.is_from_main_session() {
                    eprintln!(
                        "{}removing subgraph '{}' from this `rover dev` session",
                        Emoji::Reload,
                        &subgraph_name
                    );
                } else {
                    tracing::debug!(
                        "removing subgraph '{}' from the main `rover dev` session",
                        &subgraph_name
                    );
                }
            }
            FollowerMessageKind::Shutdown => {
                tracing::debug!("shutting down the router for this `rover dev` session");
            }
            FollowerMessageKind::HealthCheck => {
                tracing::debug!("sending health check ping to the main `rover dev` session");
            }
            FollowerMessageKind::GetVersion {
                follower_version: _,
            } => {
                tracing::debug!("requesting the version of the main `rover dev` session");
            }
            FollowerMessageKind::GetSubgraphs => {
                tracing::debug!("asking main `rover dev` session about existing subgraphs");
            }
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum FollowerMessageKind {
    GetVersion { follower_version: String },
    GetSubgraphs,
    HealthCheck,
    Shutdown,
    AddSubgraph { subgraph_entry: SubgraphEntry },
    UpdateSubgraph { subgraph_entry: SubgraphEntry },
    RemoveSubgraph { subgraph_name: SubgraphName },
}

impl FollowerMessageKind {
    fn get_version() -> Self {
        Self::GetVersion {
            follower_version: PKG_VERSION.to_string(),
        }
    }

    fn get_subgraphs() -> Self {
        Self::GetSubgraphs
    }

    fn health_check() -> Self {
        Self::HealthCheck
    }

    fn shutdown() -> Self {
        Self::Shutdown
    }

    fn add_subgraph(subgraph: &SubgraphDefinition) -> Result<Self> {
        Ok(Self::AddSubgraph {
            subgraph_entry: entry_from_definition(subgraph)?,
        })
    }

    fn update_subgraph(subgraph: &SubgraphDefinition) -> Result<Self> {
        Ok(Self::UpdateSubgraph {
            subgraph_entry: entry_from_definition(subgraph)?,
        })
    }

    fn remove_subgraph(subgraph_name: &SubgraphName) -> Self {
        Self::RemoveSubgraph {
            subgraph_name: subgraph_name.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn follower_message_can_request_version() {
        let message = FollowerMessageKind::get_version();
        let expected_message_json = serde_json::to_string(&message).unwrap();
        assert_eq!(
            expected_message_json,
            serde_json::json!({"GetVersion": {"follower_version": PKG_VERSION.to_string()}})
                .to_string()
        )
    }
}

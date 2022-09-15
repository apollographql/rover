use crate::Result;
use crate::PKG_VERSION;
use apollo_federation_types::build::SubgraphDefinition;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

use crate::command::dev::protocol::{entry_from_definition, SubgraphEntry, SubgraphName};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum FollowerMessageKind {
    AddSubgraph { subgraph_entry: SubgraphEntry },
    UpdateSubgraph { subgraph_entry: SubgraphEntry },
    RemoveSubgraph { subgraph_name: SubgraphName },
    KillRouter,
    GetSubgraphs,
    HealthCheck,
    GetVersion { follower_version: String },
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
        Self::GetVersion {
            follower_version: PKG_VERSION.to_string(),
        }
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
            Self::GetVersion {
                follower_version: _,
            } => {
                tracing::debug!("requesting the version of the main `rover dev` session");
            }
            Self::GetSubgraphs => {
                tracing::debug!("asking main `rover dev` session about existing subgraphs");
            }
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

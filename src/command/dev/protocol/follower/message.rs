use apollo_federation_types::javascript::SubgraphDefinition;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

use crate::command::dev::protocol::{entry_from_definition, SubgraphEntry, SubgraphName};
use crate::{RoverResult, PKG_VERSION};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub(crate) enum FollowerMessage {
    GetVersion { follower_version: String },
    GetSubgraphs,
    HealthCheck,
    Shutdown,
    AddSubgraph { subgraph_entry: SubgraphEntry },
    UpdateSubgraph { subgraph_entry: SubgraphEntry },
    RemoveSubgraph { subgraph_name: SubgraphName },
}

impl FollowerMessage {
    pub(crate) fn get_version() -> Self {
        Self::GetVersion {
            follower_version: PKG_VERSION.to_string(),
        }
    }

    pub(crate) fn add_subgraph(subgraph: &SubgraphDefinition) -> RoverResult<Self> {
        Ok(Self::AddSubgraph {
            subgraph_entry: entry_from_definition(subgraph)?,
        })
    }

    pub(crate) fn update_subgraph(subgraph: &SubgraphDefinition) -> RoverResult<Self> {
        Ok(Self::UpdateSubgraph {
            subgraph_entry: entry_from_definition(subgraph)?,
        })
    }

    pub(crate) fn print(&self) {
        tracing::debug!("sending message to self: {:?}", &self);
        match self {
            Self::AddSubgraph { subgraph_entry } => {
                eprintln!(
                    "starting a session with the '{}' subgraph",
                    &subgraph_entry.0 .0
                );
            }
            Self::UpdateSubgraph { subgraph_entry } => {
                eprintln!(
                    "updating the schema for the '{}' subgraph in the session",
                    &subgraph_entry.0 .0
                );
            }
            Self::RemoveSubgraph { subgraph_name } => {
                eprintln!(
                    "removing the '{}' subgraph from this session",
                    &subgraph_name
                );
            }
            Self::Shutdown => {
                tracing::debug!("shutting down the router for this session");
            }
            Self::HealthCheck => {
                tracing::debug!("sending health check ping to the main process");
            }
            Self::GetVersion {
                follower_version: _,
            } => {
                tracing::debug!("requesting the version of the main process");
            }
            Self::GetSubgraphs => {
                tracing::debug!("asking the main process about existing subgraphs");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn follower_message_can_request_version() {
        let message = FollowerMessage::get_version();
        let expected_message_json = serde_json::to_string(&message).unwrap();
        assert_eq!(
            expected_message_json,
            serde_json::json!({"GetVersion": {"follower_version": PKG_VERSION.to_string()}})
                .to_string()
        )
    }
}

use apollo_federation_types::javascript::SubgraphDefinition;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

use crate::command::dev::protocol::{entry_from_definition, SubgraphEntry, SubgraphName};
use crate::RoverResult;

/// These are the messages sent from `SubgraphWatcher` to `Orchestrator`
#[derive(Serialize, Deserialize, Debug, Clone)]
pub(crate) enum SubgraphMessage {
    AddSubgraph { subgraph_entry: SubgraphEntry },
    UpdateSubgraph { subgraph_entry: SubgraphEntry },
    // TODO: Add/remove shouldn't happen at this level
    RemoveSubgraph { subgraph_name: SubgraphName },
}

impl SubgraphMessage {
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
        }
    }
}

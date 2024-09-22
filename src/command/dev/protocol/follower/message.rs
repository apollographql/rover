use apollo_federation_types::javascript::SubgraphDefinition;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

use crate::command::dev::protocol::{entry_from_definition, SubgraphEntry, SubgraphName};
use crate::RoverResult;

/// These are the messages sent from `SubgraphWatcher` to `Orchestrator`
#[derive(Serialize, Deserialize, Debug, Clone)]
pub(crate) enum SubgraphMessage {
    Add { subgraph_entry: SubgraphEntry },
    Update { subgraph_entry: SubgraphEntry },
    // TODO: Add/remove shouldn't happen at this level
    Remove { subgraph_name: SubgraphName },
}

impl SubgraphMessage {
    pub(crate) fn add(subgraph: &SubgraphDefinition) -> RoverResult<Self> {
        Ok(Self::Add {
            subgraph_entry: entry_from_definition(subgraph)?,
        })
    }

    pub(crate) fn update(subgraph: &SubgraphDefinition) -> RoverResult<Self> {
        Ok(Self::Update {
            subgraph_entry: entry_from_definition(subgraph)?,
        })
    }

    pub(crate) fn print(&self) {
        tracing::debug!("sending message to self: {:?}", &self);
        match self {
            Self::Add { subgraph_entry } => {
                eprintln!(
                    "starting a session with the '{}' subgraph",
                    &subgraph_entry.0 .0
                );
            }
            Self::Update { subgraph_entry } => {
                eprintln!(
                    "updating the schema for the '{}' subgraph in the session",
                    &subgraph_entry.0 .0
                );
            }
            Self::Remove { subgraph_name } => {
                eprintln!(
                    "removing the '{}' subgraph from this session",
                    &subgraph_name
                );
            }
        }
    }
}

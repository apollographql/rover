use crossbeam_channel::{bounded, Receiver, Sender};
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

use crate::PKG_VERSION;

use super::types::{SubgraphKeys, SubgraphName};

#[derive(Debug, Clone)]
pub struct LeaderChannel {
    pub sender: Sender<LeaderMessageKind>,
    pub receiver: Receiver<LeaderMessageKind>,
}

impl LeaderChannel {
    pub fn new() -> Self {
        let (sender, receiver) = bounded(0);

        Self { sender, receiver }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum LeaderMessageKind {
    GetVersion {
        follower_version: String,
        leader_version: String,
    },
    LeaderSessionInfo {
        subgraphs: SubgraphKeys,
    },
    CompositionSuccess {
        action: String,
    },
    ErrorNotification {
        error: String,
    },
    MessageReceived,
}

impl LeaderMessageKind {
    // TODO: figure out if this is just for multi-terminal use case? if so, can be removed
    pub fn get_version(follower_version: &str) -> Self {
        Self::GetVersion {
            follower_version: follower_version.to_string(),
            leader_version: PKG_VERSION.to_string(),
        }
    }

    pub fn current_subgraphs(subgraphs: SubgraphKeys) -> Self {
        Self::LeaderSessionInfo { subgraphs }
    }

    pub fn error(error: String) -> Self {
        Self::ErrorNotification { error }
    }

    pub fn add_subgraph_composition_success(subgraph_name: &SubgraphName) -> Self {
        Self::CompositionSuccess {
            action: format!("adding the '{}' subgraph", subgraph_name),
        }
    }

    pub fn update_subgraph_composition_success(subgraph_name: &SubgraphName) -> Self {
        Self::CompositionSuccess {
            action: format!("updating the '{}' subgraph", subgraph_name),
        }
    }

    pub fn remove_subgraph_composition_success(subgraph_name: &SubgraphName) -> Self {
        Self::CompositionSuccess {
            action: format!("removing the '{}' subgraph", subgraph_name),
        }
    }

    pub fn message_received() -> Self {
        Self::MessageReceived
    }

    pub fn print(&self) {
        match self {
            LeaderMessageKind::ErrorNotification { error } => {
                eprintln!("{}", error);
            }
            LeaderMessageKind::CompositionSuccess { action } => {
                eprintln!("successfully composed after {}", &action);
            }
            LeaderMessageKind::LeaderSessionInfo { subgraphs } => {
                let subgraphs = match subgraphs.len() {
                    0 => "no subgraphs".to_string(),
                    1 => "1 subgraph".to_string(),
                    l => format!("{} subgraphs", l),
                };
                tracing::info!("the main `rover dev` process currently has {}", subgraphs);
            }
            LeaderMessageKind::GetVersion {
                leader_version,
                follower_version: _,
            } => {
                tracing::debug!(
                    "the main `rover dev` process is running version {}",
                    &leader_version
                );
            }
            LeaderMessageKind::MessageReceived => {
                tracing::debug!(
                        "the main `rover dev` process acknowledged the message, but did not take an action"
                    )
            }
        }
    }
}

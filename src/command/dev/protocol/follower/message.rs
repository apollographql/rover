use serde::{Deserialize, Serialize};
use std::fmt::Debug;

use crate::federation::supergraph_config::ResolvedSubgraphConfig;

/// These are the messages sent from `SubgraphWatcher` to `Orchestrator`
#[derive(Serialize, Deserialize, Debug, Clone)]
pub(crate) struct SubgraphUpdated {
    pub(crate) subgraph_name: String,
    pub(crate) subgraph_config: ResolvedSubgraphConfig,
}

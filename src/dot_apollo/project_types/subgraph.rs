use apollo_federation_types::config::SubgraphConfig;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SubgraphProjectConfig {
    supergraph: Option<String>,
    subgraph: SubgraphConfig,
}

impl SubgraphProjectConfig {
    pub(crate) fn new(supergraph: Option<String>, subgraph: SubgraphConfig) -> Self {
        Self {
            supergraph,
            subgraph,
        }
    }
}

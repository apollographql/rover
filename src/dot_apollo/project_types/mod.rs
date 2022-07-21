mod multi_subgraph;
pub(crate) use multi_subgraph::{MultiSubgraphConfig, SubgraphConfig};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "project_type", rename_all = "lowercase")]
#[non_exhaustive]
pub enum ProjectType {
    Subgraph(MultiSubgraphConfig),
}

impl ProjectType {
    pub fn get_multi_subgraph(&self) -> Option<MultiSubgraphConfig> {
        match self {
            Self::Subgraph(multi) => Some(multi.clone()),
            _ => None,
        }
    }
}

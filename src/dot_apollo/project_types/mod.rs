mod subgraph;
pub(crate) use subgraph::SubgraphProjectConfig;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "project_type", rename_all = "lowercase")]
pub enum ProjectType {
    Subgraph(SubgraphProjectConfig),
}

use saucer::{clap, Parser};
use serde::{Deserialize, Serialize};

use saucer::{anyhow, Error};

#[derive(Debug, Clone, Serialize, Deserialize, Parser)]
pub struct SubgraphOpt {
    /// Name of the subgraph to validate
    #[clap(long = "name")]
    #[serde(skip_serializing)]
    pub subgraph_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Parser)]
pub struct OptionalSubgraphOpt {
    /// Name of the subgraph to validate
    #[clap(long = "name")]
    #[serde(skip_serializing)]
    pub subgraph_name: Option<String>,
}

use serde::{Deserialize, Serialize};
use structopt::StructOpt;

#[derive(Debug, Serialize, Deserialize, StructOpt)]
pub struct SubgraphOpt {
    /// Name of the subgraph to validate
    #[structopt(long = "name")]
    #[serde(skip_serializing)]
    pub subgraph_name: String,
}

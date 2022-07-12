use rover_client::shared::GraphRef;
use saucer::{clap, Parser};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Parser)]
pub struct GraphRefOpt {
    /// <NAME>@<VARIANT> of graph in Apollo Studio to fetch from.
    /// @<VARIANT> may be left off, defaulting to @current
    #[clap(name = "GRAPH_REF")]
    #[serde(skip_serializing)]
    pub graph_ref: GraphRef,
}

#[derive(Debug, Serialize, Deserialize, Parser)]
pub struct OptionalGraphRefOpt {
    /// <NAME>@<VARIANT> of graph in Apollo Studio to fetch from.
    /// @<VARIANT> may be left off, defaulting to @current
    #[clap(name = "GRAPH_REF")]
    #[serde(skip_serializing)]
    pub graph_ref: Option<GraphRef>,
}

use clap::Parser;
use rover_client::shared::GraphRef;
use serde::Serialize;

#[derive(Debug, Serialize, Parser)]
pub struct GraphRefOpt {
    /// <NAME>@<VARIANT> of graph in Apollo Studio.
    /// @<VARIANT> may be left off, defaulting to @current
    #[arg(value_name = "GRAPH_REF")]
    #[serde(skip_serializing)]
    pub graph_ref: GraphRef,
}

#[derive(Debug, Serialize, Parser)]
pub struct OptionalGraphRefOpt {
    /// <NAME>@<VARIANT> of graph in Apollo Studio.
    /// @<VARIANT> may be left off, defaulting to @current
    #[arg(value_name = "GRAPH_REF")]
    #[serde(skip_serializing)]
    pub graph_ref: Option<GraphRef>,
}

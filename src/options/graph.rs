use clap::Parser;
use rover_studio::types::GraphRef;
use serde::Deserialize;

#[derive(Debug, Deserialize, Parser)]
pub struct GraphRefOpt {
    /// <NAME>@<VARIANT> of graph in Apollo Studio.
    /// @<VARIANT> may be left off, defaulting to @current
    #[arg(value_name = "GRAPH_REF")]
    pub graph_ref: GraphRef,
}

#[derive(Debug, Deserialize, Parser)]
pub struct OptionalGraphRefOpt {
    /// <NAME>@<VARIANT> of graph in Apollo Studio.
    /// @<VARIANT> may be left off, defaulting to @current
    #[arg(value_name = "GRAPH_REF")]
    pub graph_ref: Option<GraphRef>,
}

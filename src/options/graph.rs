use rover_client::shared::GraphRef;
use saucer::{clap, Parser};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Parser)]
pub struct GraphRefOpt {
    /// <NAME>@<VARIANT> of graph in Apollo Studio.
    /// @<VARIANT> may be left off, defaulting to @current
    #[clap(name = "GRAPH_REF")]
    #[serde(skip_serializing)]
    pub graph_ref: GraphRef,
}

#[derive(Debug, Serialize, Deserialize, Parser)]
pub struct OptionalGraphRefOpt {
    /// <NAME>@<VARIANT> of graph in Apollo Studio.
    /// @<VARIANT> may be left off, defaulting to @current
    #[clap(name = "GRAPH_REF", conflicts_with("variant"))]
    #[serde(skip_serializing)]
    pub graph_ref: Option<GraphRef>,

    /// A variant of a graph in Apollo Studio.
    ///
    /// This option can only be used in directories
    /// with a .apollo directory, and cannot be used
    /// with the positional graph ref argument.
    #[clap(long)]
    variant: Option<String>,
}

impl OptionalGraphRefOpt {
    pub fn variant(&self) -> Option<String> {
        if let Some(variant) = &self.variant {
            Some(variant.to_string())
        } else if let Some(graph_ref) = &self.graph_ref {
            Some(graph_ref.variant.to_string())
        } else {
            None
        }
    }

    pub fn graph_id(&self) -> Option<String> {
        if let Some(graph_ref) = &self.graph_ref {
            Some(graph_ref.name.to_string())
        } else {
            None
        }
    }
}

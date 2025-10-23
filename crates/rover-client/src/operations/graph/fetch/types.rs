use crate::{operations::graph::fetch::runner::graph_fetch_query, shared::GraphRef};

type QueryVariables = graph_fetch_query::Variables;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct GraphFetchInput {
    pub graph_ref: GraphRef,
}

impl From<GraphFetchInput> for QueryVariables {
    fn from(input: GraphFetchInput) -> Self {
        Self {
            graph_id: input.graph_ref.name,
            variant: input.graph_ref.variant,
        }
    }
}

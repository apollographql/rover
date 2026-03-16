use rover_studio::types::GraphRef;

use crate::operations::graph::fetch::runner::graph_fetch_query;

type QueryVariables = graph_fetch_query::Variables;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct GraphFetchInput {
    pub graph_ref: GraphRef,
}

impl From<GraphFetchInput> for QueryVariables {
    fn from(input: GraphFetchInput) -> Self {
        let (name, variant) = input.graph_ref.dissolve();
        Self {
            graph_id: name.into_owned(),
            variant: variant.into_owned(),
        }
    }
}

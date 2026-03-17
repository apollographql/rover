use rover_studio::types::GraphRef;

use crate::operations::supergraph::fetch::runner::supergraph_fetch_query;

type QueryVariables = supergraph_fetch_query::Variables;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SupergraphFetchInput {
    pub graph_ref: GraphRef,
}

impl From<SupergraphFetchInput> for QueryVariables {
    fn from(input: SupergraphFetchInput) -> Self {
        let (graph_id, variant) = input.graph_ref.into_parts();
        Self { graph_id, variant }
    }
}

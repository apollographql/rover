use rover_studio::types::GraphRef;

use crate::operations::supergraph::fetch::runner::supergraph_fetch_query;

type QueryVariables = supergraph_fetch_query::Variables;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SupergraphFetchInput {
    pub graph_ref: GraphRef,
}

impl From<SupergraphFetchInput> for QueryVariables {
    fn from(input: SupergraphFetchInput) -> Self {
        let (name, variant) = input.graph_ref.dissolve();
        Self {
            graph_id: name.into_owned(),
            variant: variant.into_owned(),
        }
    }
}

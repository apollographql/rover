use crate::shared::GraphRef;

use super::runner::subgraph_fetch_query;

pub(crate) type SubgraphFetchResponseData = subgraph_fetch_query::ResponseData;
pub(crate) type SubgraphFetchGraphVariant = subgraph_fetch_query::SubgraphFetchQueryVariant;
pub(crate) type QueryVariables = subgraph_fetch_query::Variables;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SubgraphFetchInput {
    pub graph_ref: GraphRef,
    pub subgraph_name: String,
}

impl From<SubgraphFetchInput> for QueryVariables {
    fn from(input: SubgraphFetchInput) -> Self {
        Self {
            graph_ref: input.graph_ref.to_string(),
            subgraph_name: input.subgraph_name,
        }
    }
}

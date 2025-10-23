use super::service::SubgraphFetchRequest;
use crate::shared::GraphRef;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SubgraphFetchInput {
    pub graph_ref: GraphRef,
    pub subgraph_name: String,
}

impl From<SubgraphFetchInput> for SubgraphFetchRequest {
    fn from(input: SubgraphFetchInput) -> Self {
        Self::builder()
            .graph_ref(input.graph_ref)
            .subgraph_name(input.subgraph_name)
            .build()
    }
}

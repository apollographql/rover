use super::service::SupergraphFetchRequest;
use rover_studio::types::GraphRef;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SupergraphFetchInput {
    pub graph_ref: GraphRef,
}

impl From<SupergraphFetchInput> for SupergraphFetchRequest {
    fn from(input: SupergraphFetchInput) -> Self {
        Self::from(input.graph_ref)
    }
}

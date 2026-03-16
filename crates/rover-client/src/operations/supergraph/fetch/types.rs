use rover_studio::types::GraphRef;

use super::service::SupergraphFetchRequest;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SupergraphFetchInput {
    pub graph_ref: GraphRef,
}

impl From<SupergraphFetchInput> for SupergraphFetchRequest {
    fn from(input: SupergraphFetchInput) -> Self {
        Self::from(input.graph_ref)
    }
}

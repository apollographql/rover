use crate::{operations::graph::delete::runner::graph_delete_mutation, shared::GraphRef};

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct GraphDeleteInput {
    pub graph_ref: GraphRef,
}

type MutationVariables = graph_delete_mutation::Variables;
impl From<GraphDeleteInput> for MutationVariables {
    fn from(input: GraphDeleteInput) -> Self {
        Self {
            graph_id: input.graph_ref.name,
            variant: input.graph_ref.variant,
        }
    }
}

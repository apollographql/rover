use rover_studio::types::GraphRef;

use crate::operations::graph::delete::runner::graph_delete_mutation;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct GraphDeleteInput {
    pub graph_ref: GraphRef,
}

type MutationVariables = graph_delete_mutation::Variables;
impl From<GraphDeleteInput> for MutationVariables {
    fn from(input: GraphDeleteInput) -> Self {
        let (name, variant) = input.graph_ref.dissolve();
        Self {
            graph_id: name.into_owned(),
            variant: variant.into_owned(),
        }
    }
}

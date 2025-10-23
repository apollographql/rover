use crate::{operations::graph::variant::runner::variant_list_query, shared::GraphRef};

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct VariantListInput {
    pub graph_ref: GraphRef,
}

type MutationVariables = variant_list_query::Variables;
impl From<VariantListInput> for MutationVariables {
    fn from(input: VariantListInput) -> Self {
        Self {
            graph_id: input.graph_ref.name,
        }
    }
}

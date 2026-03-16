use rover_studio::types::GraphRef;

use crate::operations::graph::variant::runner::variant_list_query;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct VariantListInput {
    pub graph_ref: GraphRef,
}

type MutationVariables = variant_list_query::Variables;
impl From<VariantListInput> for MutationVariables {
    fn from(input: VariantListInput) -> Self {
        let (name, _variant) = input.graph_ref.dissolve();
        Self {
            graph_id: name.into_owned(),
        }
    }
}

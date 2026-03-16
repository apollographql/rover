use rover_studio::types::GraphRef;

use crate::operations::config::is_federated::runner::is_federated_graph;

type QueryVariables = is_federated_graph::Variables;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct IsFederatedInput {
    pub graph_ref: GraphRef,
}

impl From<IsFederatedInput> for QueryVariables {
    fn from(input: IsFederatedInput) -> Self {
        let (name, variant) = input.graph_ref.dissolve();
        Self {
            graph_id: name.into_owned(),
            variant: variant.into_owned(),
        }
    }
}

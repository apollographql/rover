use crate::{operations::config::is_federated::runner::is_federated_graph, shared::GraphRef};

type QueryVariables = is_federated_graph::Variables;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct IsFederatedInput {
    pub graph_ref: GraphRef,
}

impl From<IsFederatedInput> for QueryVariables {
    fn from(input: IsFederatedInput) -> Self {
        Self {
            graph_id: input.graph_ref.name,
            variant: input.graph_ref.variant,
        }
    }
}

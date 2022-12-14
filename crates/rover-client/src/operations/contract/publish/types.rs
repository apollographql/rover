use super::runner::contract_publish_mutation;

use crate::shared::GraphRef;

pub(crate) type MutationVariables = contract_publish_mutation::Variables;

use serde::Serialize;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ContractPublishInput {
    pub contract_ref: GraphRef,
}

impl From<ContractPublishInput> for MutationVariables {
    fn from(publish_input: ContractPublishInput) -> Self {
        Self {
            graph_id: publish_input.contract_ref.name,
            // variant: publish_input.contract_ref.variant,
        }
    }
}

#[derive(Debug, Clone, Serialize, Eq, PartialEq)]
pub struct ContractPublishResponse {
    pub graph: GraphRef,
}

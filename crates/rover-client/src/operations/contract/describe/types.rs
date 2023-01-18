use crate::{operations::contract::describe::runner::contract_describe_query, shared::GraphRef};

pub(crate) type QueryResponseData = contract_describe_query::ResponseData;

type QueryVariables = contract_describe_query::Variables;

use serde::Serialize;

#[derive(Clone, Eq, PartialEq, Debug)]
pub struct ContractDescribeInput {
    pub graph_ref: GraphRef,
}

impl From<ContractDescribeInput> for QueryVariables {
    fn from(input: ContractDescribeInput) -> Self {
        Self {
            graph_id: input.graph_ref.name,
            variant: input.graph_ref.variant,
        }
    }
}

#[derive(Clone, Serialize, Eq, PartialEq, Debug)]
pub struct ContractDescribeResponse {
    pub description: String,

    #[serde(skip_serializing)]
    pub root_url: String,

    #[serde(skip_serializing)]
    pub graph_ref: GraphRef,
}

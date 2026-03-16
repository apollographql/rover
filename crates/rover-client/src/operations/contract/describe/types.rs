use crate::operations::contract::describe::runner::contract_describe_query;

pub(crate) type QueryResponseData = contract_describe_query::ResponseData;

type QueryVariables = contract_describe_query::Variables;

use rover_studio::types::GraphRef;
use serde::Serialize;

#[derive(Clone, Eq, PartialEq, Debug)]
pub struct ContractDescribeInput {
    pub graph_ref: GraphRef,
}

impl From<ContractDescribeInput> for QueryVariables {
    fn from(input: ContractDescribeInput) -> Self {
        let (name, variant) = input.graph_ref.dissolve();
        Self {
            graph_id: name.into_owned(),
            variant: variant.into_owned(),
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

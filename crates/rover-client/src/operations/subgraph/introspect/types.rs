use crate::operations::subgraph::introspect::runner::subgraph_introspect_query;

pub(crate) type QueryVariables = subgraph_introspect_query::Variables;
pub(crate) type QueryResponseData = subgraph_introspect_query::ResponseData;

use std::collections::HashMap;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SubgraphIntrospectInput {
    pub headers: HashMap<String, String>,
}

impl From<SubgraphIntrospectInput> for QueryVariables {
    fn from(_input: SubgraphIntrospectInput) -> Self {
        Self {}
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SubgraphIntrospectResponse {
    pub result: String,
}

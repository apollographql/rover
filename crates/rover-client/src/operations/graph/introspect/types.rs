use std::collections::HashMap;

use crate::operations::graph::introspect::runner::graph_introspect_query;

pub(crate) type QueryResponseData = graph_introspect_query::ResponseData;
pub(crate) type QueryVariables = graph_introspect_query::Variables;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphIntrospectInput {
    pub headers: HashMap<String, String>,
}

impl From<GraphIntrospectInput> for QueryVariables {
    fn from(_input: GraphIntrospectInput) -> Self {
        Self {}
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphIntrospectResponse {
    pub schema_sdl: String,
}

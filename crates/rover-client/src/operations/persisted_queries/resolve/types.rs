use rover_studio::types::GraphRef;

use crate::operations::persisted_queries::resolve::runner::resolve_persisted_query_list_query;

type QueryVariables = resolve_persisted_query_list_query::Variables;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ResolvePersistedQueryListInput {
    pub graph_ref: GraphRef,
}

impl From<ResolvePersistedQueryListInput> for QueryVariables {
    fn from(input: ResolvePersistedQueryListInput) -> Self {
        let (graph_id, variant) = input.graph_ref.into_parts();
        Self { graph_id, variant }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PersistedQueryList {
    pub graph_ref: GraphRef,
    pub id: String,
    pub name: String,
}

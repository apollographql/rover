use crate::{
    operations::persisted_queries::resolve::runner::resolve_persisted_query_list_query,
    shared::GraphRef,
};

type QueryVariables = resolve_persisted_query_list_query::Variables;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ResolvePersistedQueryListInput {
    pub graph_ref: GraphRef,
}

impl From<ResolvePersistedQueryListInput> for QueryVariables {
    fn from(input: ResolvePersistedQueryListInput) -> Self {
        Self {
            graph_id: input.graph_ref.name,
            variant: input.graph_ref.variant,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PersistedQueryList {
    pub graph_ref: GraphRef,
    pub id: String,
    pub name: String,
}

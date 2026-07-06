use crate::operations::persisted_queries::name::runner::persisted_query_list_name_query;

type QueryVariables = persisted_query_list_name_query::Variables;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct PersistedQueryListNameInput {
    pub graph_id: String,
    pub list_id: String,
}

impl From<PersistedQueryListNameInput> for QueryVariables {
    fn from(input: PersistedQueryListNameInput) -> Self {
        Self {
            graph_id: input.graph_id,
            list_id: input.list_id,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PersistedQueryListNameResponse {
    pub name: String,
}

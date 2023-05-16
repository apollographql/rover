use crate::operations::queries::persist::runner::queries_persist_mutation;
use crate::shared::GraphRef;

type QueryVariables = queries_persist_mutation::Variables;
type Timestamp = String;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct QueriesPersistInput {
    pub graph_ref: GraphRef,
}

impl From<QueriesPersistInput> for QueryVariables {
    fn from(input: QueriesPersistInput) -> Self {
        Self {
            graph_id: input.graph_ref.name,
            variant: input.graph_ref.variant,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QueriesPersistResponse {
    pub graph_ref: GraphRef,
}

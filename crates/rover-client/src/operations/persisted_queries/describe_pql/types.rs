use crate::operations::persisted_queries::describe_pql::runner::describe_persisted_query_list_query;
use crate::shared::GraphRef;

type QueryVariables = describe_persisted_query_list_query::Variables;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct DescribePQLInput {
    pub graph_ref: GraphRef,
}

impl From<DescribePQLInput> for QueryVariables {
    fn from(input: DescribePQLInput) -> Self {
        Self {
            graph_id: input.graph_ref.name,
            variant: input.graph_ref.variant,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DescribePQLResponse {
    pub graph_ref: GraphRef,
    pub id: String,
}

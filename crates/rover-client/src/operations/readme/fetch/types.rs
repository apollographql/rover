use crate::operations::readme::fetch::runner::readme_fetch_query;
use crate::shared::GraphRef;

type QueryVariables = readme_fetch_query::Variables;

#[derive(Debug, Clone, PartialEq)]
pub struct ReadmeFetchInput {
    pub graph_ref: GraphRef,
}

impl From<ReadmeFetchInput> for QueryVariables {
    fn from(input: ReadmeFetchInput) -> Self {
        Self {
            graph_id: input.graph_ref.name,
            variant: input.graph_ref.variant,
        }
    }
}

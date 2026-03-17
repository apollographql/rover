use rover_studio::types::GraphRef;

use crate::operations::readme::fetch::runner::readme_fetch_query;

type QueryVariables = readme_fetch_query::Variables;
type Timestamp = String;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ReadmeFetchInput {
    pub graph_ref: GraphRef,
}

impl From<ReadmeFetchInput> for QueryVariables {
    fn from(input: ReadmeFetchInput) -> Self {
        let (graph_id, variant) = input.graph_ref.into_parts();
        Self { graph_id, variant }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReadmeFetchResponse {
    pub graph_ref: GraphRef,
    pub content: String,
    pub last_updated_time: Option<Timestamp>,
}

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
        let (name, variant) = input.graph_ref.dissolve();
        Self {
            graph_id: name.into_owned(),
            variant: variant.into_owned(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReadmeFetchResponse {
    pub graph_ref: GraphRef,
    pub content: String,
    pub last_updated_time: Option<Timestamp>,
}

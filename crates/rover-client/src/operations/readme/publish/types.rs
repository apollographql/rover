use crate::{operations::readme::publish::runner::readme_publish_mutation, shared::GraphRef};

type QueryVariables = readme_publish_mutation::Variables;
type Timestamp = String;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ReadmePublishInput {
    pub graph_ref: GraphRef,
    pub readme: String,
}

impl From<ReadmePublishInput> for QueryVariables {
    fn from(input: ReadmePublishInput) -> Self {
        Self {
            graph_id: input.graph_ref.name,
            variant: input.graph_ref.variant,
            readme: input.readme,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReadmePublishResponse {
    pub graph_ref: GraphRef,
    pub new_content: String,
    pub last_updated_time: Option<Timestamp>,
}

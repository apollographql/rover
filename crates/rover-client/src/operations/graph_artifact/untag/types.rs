use serde::Serialize;

use crate::operations::graph_artifact::untag::runner::delete_graph_artifact_tag_mutation;

type QueryVariables = delete_graph_artifact_tag_mutation::Variables;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeleteGraphArtifactTagInput {
    pub graph_id: String,
    pub tag: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DeleteGraphArtifactTagResponse {
    pub graph_id: String,
    pub tag: String,
}

impl From<DeleteGraphArtifactTagInput> for QueryVariables {
    fn from(input: DeleteGraphArtifactTagInput) -> Self {
        Self {
            graph_id: input.graph_id,
            tag: input.tag,
        }
    }
}

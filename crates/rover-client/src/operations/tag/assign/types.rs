use serde::Serialize;

use crate::operations::tag::assign::runner::assign_graph_artifact_tag_mutation;

type QueryVariables = assign_graph_artifact_tag_mutation::Variables;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GraphArtifactInput {
    pub digest: Option<String>,
    pub id: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssignGraphArtifactTagInput {
    pub graph_id: String,
    pub artifact: GraphArtifactInput,
    pub tag: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AssignGraphArtifactTagResponse {
    pub graph_id: String,
    pub tag: String,
    pub graph_artifact_id: String,
    pub digest: String,
}

impl From<AssignGraphArtifactTagInput> for QueryVariables {
    fn from(input: AssignGraphArtifactTagInput) -> Self {
        Self {
            graph_id: input.graph_id,
            artifact: assign_graph_artifact_tag_mutation::GraphArtifactInput {
                digest: input.artifact.digest,
                id: input.artifact.id,
            },
            tag: input.tag,
        }
    }
}

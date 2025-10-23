use super::runner::subgraph_publish_mutation;
use crate::shared::{GitContext, GraphRef};

pub(crate) type ResponseData = subgraph_publish_mutation::ResponseData;
pub(crate) type MutationVariables = subgraph_publish_mutation::Variables;
pub(crate) type UpdateResponse =
    subgraph_publish_mutation::SubgraphPublishMutationGraphPublishSubgraph;

use apollo_federation_types::rover::BuildErrors;

type SchemaInput = subgraph_publish_mutation::PartialSchemaInput;
type GitContextInput = subgraph_publish_mutation::GitContextInput;

use serde::Serialize;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SubgraphPublishInput {
    pub graph_ref: GraphRef,
    pub subgraph: String,
    pub url: Option<String>,
    pub schema: String,
    pub git_context: GitContext,
    pub convert_to_federated_graph: bool,
}

#[derive(Debug, Clone, Serialize, Eq, PartialEq)]
pub struct SubgraphPublishResponse {
    pub api_schema_hash: Option<String>,

    pub supergraph_was_updated: bool,

    pub subgraph_was_created: bool,

    pub subgraph_was_updated: bool,

    #[serde(skip_serializing)]
    pub build_errors: BuildErrors,

    pub launch_url: Option<String>,

    pub launch_cli_copy: Option<String>,
}

impl From<SubgraphPublishInput> for MutationVariables {
    fn from(publish_input: SubgraphPublishInput) -> Self {
        Self {
            graph_id: publish_input.graph_ref.name,
            variant: publish_input.graph_ref.variant,
            subgraph: publish_input.subgraph,
            url: publish_input.url,
            schema: SchemaInput {
                sdl: Some(publish_input.schema),
                hash: None,
            },
            git_context: publish_input.git_context.into(),
            revision: "".to_string(),
        }
    }
}

impl From<GitContext> for GitContextInput {
    fn from(git_context: GitContext) -> GitContextInput {
        GitContextInput {
            branch: git_context.branch,
            commit: git_context.commit,
            committer: git_context.author,
            remote_url: git_context.remote_url,
            message: None,
        }
    }
}

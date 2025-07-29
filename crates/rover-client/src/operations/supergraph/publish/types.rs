use super::runner::supergraph_publish_mutation;

use crate::shared::{GitContext, GraphRef};

pub(crate) type ResponseData = supergraph_publish_mutation::ResponseData;
pub(crate) type MutationVariables = supergraph_publish_mutation::Variables;
pub(crate) type UpdateResponse =
    supergraph_publish_mutation::SupergraphPublishMutationGraphPublishSubgraphs;

use apollo_federation_types::rover::BuildErrors;

type SchemaInput = supergraph_publish_mutation::PartialSchemaInput;
type GitContextInput = supergraph_publish_mutation::GitContextInput;

use serde::Serialize;

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
pub struct SupergraphPublishInput {
    pub graph_ref: GraphRef,
    pub git_context: GitContext,
    pub subgraph_inputs: Vec<SupergraphPublishSubgraphInput>,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
pub struct SupergraphPublishSubgraphInput {
    pub subgraph: String,
    pub url: Option<String>,
    pub schema: String,
}

#[derive(Debug, Clone, Serialize, Eq, PartialEq)]
pub struct SupergraphPublishResponse {
    pub api_schema_hash: Option<String>,

    pub supergraph_was_updated: bool,

    pub subgraphs_created: Vec<String>,

    pub subgraphs_updated: Vec<String>,

    #[serde(skip_serializing)]
    pub build_errors: BuildErrors,

    pub launch_url: Option<String>,

    pub launch_cli_copy: Option<String>,
}

impl From<SupergraphPublishInput> for MutationVariables {
    fn from(publish_input: SupergraphPublishInput) -> Self {
        Self {
            graph_id: publish_input.graph_ref.name,
            variant: publish_input.graph_ref.variant,
            subgraph_inputs: publish_input
                .subgraph_inputs
                .into_iter()
                .map(
                    |input| supergraph_publish_mutation::PublishSubgraphsSubgraphInput {
                        name: input.subgraph,
                        url: input.url,
                        active_partial_schema: SchemaInput {
                            sdl: Some(input.schema),
                            hash: None,
                        },
                    },
                )
                .collect(),
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

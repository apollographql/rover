use super::runner::subgraphs_publish_mutation;
use apollo_federation_types::rover::BuildErrors;

use crate::shared::{GitContext, GraphRef};

pub(crate) type ResponseData = subgraphs_publish_mutation::ResponseData;
pub(crate) type MutationVariables = subgraphs_publish_mutation::Variables;
pub(crate) type UpdateResponse =
    subgraphs_publish_mutation::SubgraphsPublishMutationGraphPublishSubgraphs;
type SchemaInput = subgraphs_publish_mutation::PartialSchemaInput;
type GitContextInput = subgraphs_publish_mutation::GitContextInput;
type PublishSubgraphsSubgraphInput = subgraphs_publish_mutation::PublishSubgraphsSubgraphInput;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SubgraphsPublishInput {
    pub graph_ref: GraphRef,
    pub subgraph_manifest: SubgraphManifest,
    pub git_context: GitContext,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct SubgraphManifest {
    pub subgraph_inputs: Vec<SubgraphPublishInput>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Ord, PartialOrd)]
#[serde(rename_all = "camelCase")]
pub struct SubgraphPublishInput {
    pub subgraph: String,
    pub url: Option<String>,
    pub schema: String,
    #[serde(default)]
    pub no_url: bool,
    #[serde(default)]
    pub allow_invalid_routing_url: bool,
}

impl SubgraphManifest {
    pub fn get_subgraph_names(&self) -> Vec<String> {
        self.subgraph_inputs
            .iter()
            .map(|input| input.subgraph.clone())
            .collect()
    }
}

#[derive(Debug, Clone, Serialize, Eq, PartialEq)]
pub struct SubgraphsPublishResponse {
    pub api_schema_hash: Option<String>,

    pub supergraph_was_updated: bool,

    pub subgraph_was_created: bool,

    pub subgraph_was_updated: bool,

    pub subgraphs_created: Vec<String>,

    pub subgraphs_updated: Vec<String>,

    #[serde(skip_serializing)]
    pub build_errors: BuildErrors,

    pub launch_url: Option<String>,

    pub launch_cli_copy: Option<String>,
}

impl From<SubgraphsPublishInput> for MutationVariables {
    fn from(publish_input: SubgraphsPublishInput) -> Self {
        Self {
            graph_id: publish_input.graph_ref.name,
            graph_variant: publish_input.graph_ref.variant,
            subgraph_inputs: publish_input
                .subgraph_manifest
                .subgraph_inputs
                .iter()
                .cloned()
                .map(|subgraph| PublishSubgraphsSubgraphInput {
                    active_partial_schema: SchemaInput {
                        sdl: Some(subgraph.schema),
                        hash: None,
                    },
                    name: subgraph.subgraph,
                    url: subgraph.url,
                })
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

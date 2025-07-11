use super::runner::supergraph_check_mutation;

use crate::{
    operations::supergraph::check::runner::supergraph_check_mutation::HistoricQueryParametersInput,
    shared::{CheckConfig, GitContext, GraphRef},
};

use supergraph_check_mutation::MultiSubgraphCheckAsyncInput;
pub(crate) type ResponseData = supergraph_check_mutation::ResponseData;
pub(crate) type MutationVariables = supergraph_check_mutation::Variables;

type SchemaInput = supergraph_check_mutation::SubgraphSdlCheckInput;
type GitContextInput = supergraph_check_mutation::GitContextInput;

use serde::Serialize;

#[derive(Debug, Clone, PartialEq)]
pub struct SupergraphCheckInput {
    pub graph_ref: GraphRef,
    pub git_context: GitContext,
    pub subgraphs_to_check: Vec<SupergraphCheckSubgraphInput>,
    pub config: CheckConfig,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
pub struct SupergraphCheckSubgraphInput {
    pub name: String,
    pub sdl: String,
}

#[derive(Debug, Clone, Serialize, Eq, PartialEq)]
pub struct SupergraphCheckResponse {
    pub workflow_id: String,
    pub target_url: String,
}

impl From<SupergraphCheckInput> for MutationVariables {
    fn from(input: SupergraphCheckInput) -> Self {
        Self {
            graph_id: input.graph_ref.name,
            name: input.graph_ref.variant,
            input: MultiSubgraphCheckAsyncInput {
                graph_ref: None,
                introspection_endpoint: None,
                is_proposal: Some(false),
                source_variant: None,
                triggered_by: None,
                config: input.config.into(),
                git_context: input.git_context.into(),
                is_sandbox: true,
                subgraphs_to_check: input
                    .subgraphs_to_check
                    .iter()
                    .map(|subgraph| {
                        Some(SchemaInput {
                            name: subgraph.name.clone(),
                            sdl: subgraph.sdl.clone(),
                        })
                    })
                    .collect(),
            },
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

impl From<CheckConfig> for HistoricQueryParametersInput {
    fn from(input: CheckConfig) -> Self {
        let (from, to) = match input.validation_period {
            Some(validation_period) => (
                Some(validation_period.from.to_string()),
                Some(validation_period.to.to_string()),
            ),
            None => (None, None),
        };
        Self {
            query_count_threshold: input.query_count_threshold,
            query_count_threshold_percentage: input.query_count_threshold_percentage,
            from,
            to,
            // we don't support configuring these, but we can't leave them out
            excluded_clients: None,
            excluded_operation_names: None,
            ignored_operations: None,
            included_variants: None,
        }
    }
}

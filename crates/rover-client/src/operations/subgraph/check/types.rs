use crate::operations::subgraph::check::runner::subgraph_check_mutation;
use crate::shared::{CheckConfig, GitContext, GraphRef};

type MutationInput = subgraph_check_mutation::SubgraphCheckAsyncInput;
type MutationConfig = subgraph_check_mutation::HistoricQueryParametersInput;
type MutationGitContextInput = subgraph_check_mutation::GitContextInput;
type MutationVariables = subgraph_check_mutation::Variables;
pub(crate) type MutationResponseData = subgraph_check_mutation::ResponseData;

#[derive(Debug, Clone, PartialEq)]
pub struct SubgraphCheckAsyncInput {
    pub graph_ref: GraphRef,
    pub proposed_schema: String,
    pub git_context: GitContext,
    pub config: CheckConfig,
    pub subgraph: String,
}

impl From<SubgraphCheckAsyncInput> for MutationVariables {
    fn from(input: SubgraphCheckAsyncInput) -> Self {
        let graph_ref = input.graph_ref.clone();
        Self {
            graph_id: input.graph_ref.name,
            name: input.graph_ref.variant,
            input: MutationInput {
                graph_ref: Some(graph_ref.to_string()),
                proposed_schema: input.proposed_schema,
                git_context: input.git_context.into(),
                config: input.config.into(),
                subgraph_name: input.subgraph,
                is_sandbox: false,
                introspection_endpoint: None,
                is_proposal: Some(false),
            },
        }
    }
}

impl From<CheckConfig> for MutationConfig {
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

impl From<GitContext> for MutationGitContextInput {
    fn from(git_context: GitContext) -> MutationGitContextInput {
        MutationGitContextInput {
            branch: git_context.branch,
            commit: git_context.commit,
            committer: git_context.author,
            remote_url: git_context.remote_url,
            message: None,
        }
    }
}

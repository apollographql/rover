use crate::{
    operations::graph::check::runner::graph_check_mutation,
    shared::{CheckConfig, GitContext, GraphRef},
};

type MutationInput = graph_check_mutation::CheckSchemaAsyncInput;
type MutationConfig = graph_check_mutation::HistoricQueryParametersInput;
type MutationGitContextInput = graph_check_mutation::GitContextInput;
type MutationVariables = graph_check_mutation::Variables;
pub(crate) type MutationResponseData = graph_check_mutation::ResponseData;

#[derive(Debug, Clone, PartialEq)]
pub struct CheckSchemaAsyncInput {
    pub graph_ref: GraphRef,
    pub proposed_schema: String,
    pub git_context: GitContext,
    pub config: CheckConfig,
}

impl From<CheckSchemaAsyncInput> for MutationVariables {
    fn from(input: CheckSchemaAsyncInput) -> Self {
        let graph_ref = input.graph_ref.clone();
        Self {
            graph_id: input.graph_ref.name,
            name: input.graph_ref.variant,
            input: MutationInput {
                graph_ref: Some(graph_ref.to_string()),
                proposed_schema_document: Some(input.proposed_schema),
                git_context: input.git_context.into(),
                config: input.config.into(),
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

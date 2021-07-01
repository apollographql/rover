use crate::operations::subgraph::check::mutation_runner::subgraph_check_mutation;
use crate::shared::{ChangeSeverity, CheckConfig, GitContext};

type MutationVariables = subgraph_check_mutation::Variables;

pub(crate) type MutationResponseData = subgraph_check_mutation::ResponseData;
pub(crate) type MutationCompositionErrors =
    subgraph_check_mutation::SubgraphCheckMutationServiceCheckPartialSchemaCompositionValidationResultErrors;

type MutationSchema = subgraph_check_mutation::PartialSchemaInput;
type MutationConfig = subgraph_check_mutation::HistoricQueryParameters;

pub(crate) type MutationChangeSeverity = subgraph_check_mutation::ChangeSeverity;
impl From<MutationChangeSeverity> for ChangeSeverity {
    fn from(severity: MutationChangeSeverity) -> Self {
        match severity {
            MutationChangeSeverity::NOTICE => ChangeSeverity::PASS,
            MutationChangeSeverity::FAILURE => ChangeSeverity::FAIL,
            _ => ChangeSeverity::unreachable(),
        }
    }
}

type MutationGitContextInput = subgraph_check_mutation::GitContextInput;
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

#[derive(Debug, Clone, PartialEq)]
pub struct SubgraphCheckInput {
    pub graph_id: String,
    pub variant: String,
    pub subgraph: String,
    pub proposed_schema: String,
    pub git_context: GitContext,
    pub config: CheckConfig,
}

impl From<SubgraphCheckInput> for MutationVariables {
    fn from(input: SubgraphCheckInput) -> Self {
        Self {
            graph_id: input.graph_id,
            variant: input.variant,
            subgraph: input.subgraph,
            proposed_schema: MutationSchema {
                sdl: Some(input.proposed_schema),
                hash: None,
            },
            config: MutationConfig {
                query_count_threshold: input.config.query_count_threshold,
                query_count_threshold_percentage: input.config.query_count_threshold_percentage,
                from: input.config.validation_period_from,
                to: input.config.validation_period_to,
                // we don't support configuring these, but we can't leave them out
                excluded_clients: None,
                ignored_operations: None,
                included_variants: None,
            },
            git_context: input.git_context.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompositionError {
    pub message: String,
    pub code: Option<String>,
}

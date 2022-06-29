use crate::operations::graph::check::runner::graph_check_mutation;
use crate::shared::{ChangeSeverity, CheckConfig, GitContext, GraphRef, SchemaChange};

#[derive(Debug, Clone, PartialEq)]
pub struct GraphCheckInput {
    pub graph_ref: GraphRef,
    pub proposed_schema: String,
    pub git_context: GitContext,
    pub config: CheckConfig,
}

impl From<GraphCheckInput> for MutationVariables {
    fn from(input: GraphCheckInput) -> Self {
        Self {
            graph_id: input.graph_ref.name,
            variant: Some(input.graph_ref.variant),
            proposed_schema: Some(input.proposed_schema),
            config: input.config.into(),
            git_context: input.git_context.into(),
        }
    }
}

type MutationConfig = graph_check_mutation::HistoricQueryParameters;
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

type MutationVariables = graph_check_mutation::Variables;
pub(crate) type MutationResponseData = graph_check_mutation::ResponseData;

pub(crate) type MutationChangeSeverity = graph_check_mutation::ChangeSeverity;
impl From<MutationChangeSeverity> for ChangeSeverity {
    fn from(severity: MutationChangeSeverity) -> Self {
        match severity {
            MutationChangeSeverity::NOTICE => ChangeSeverity::PASS,
            MutationChangeSeverity::FAILURE => ChangeSeverity::FAIL,
            _ => ChangeSeverity::unreachable(),
        }
    }
}

impl From<ChangeSeverity> for MutationChangeSeverity {
    fn from(severity: ChangeSeverity) -> Self {
        match severity {
            ChangeSeverity::PASS => MutationChangeSeverity::NOTICE,
            ChangeSeverity::FAIL => MutationChangeSeverity::FAILURE,
        }
    }
}

type MutationSchemaChange =
    graph_check_mutation::GraphCheckMutationGraphCheckSchemaDiffToPreviousChanges;
impl From<SchemaChange> for MutationSchemaChange {
    fn from(schema_change: SchemaChange) -> MutationSchemaChange {
        MutationSchemaChange {
            severity: schema_change.severity.into(),
            code: schema_change.code,
            description: schema_change.description,
        }
    }
}

impl From<MutationSchemaChange> for SchemaChange {
    fn from(schema_change: MutationSchemaChange) -> SchemaChange {
        SchemaChange {
            severity: schema_change.severity.into(),
            code: schema_change.code,
            description: schema_change.description,
        }
    }
}

type MutationGitContextInput = graph_check_mutation::GitContextInput;
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

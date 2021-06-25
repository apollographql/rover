use std::fmt;

use crate::utils::GitContext;

use super::query_runner::subgraph_check_query;

pub(crate) type Timestamp = String;
type QueryVariables = subgraph_check_query::Variables;
type QueryChangeSeverity = subgraph_check_query::ChangeSeverity;
type QuerySchema = subgraph_check_query::PartialSchemaInput;
type QueryConfig = subgraph_check_query::HistoricQueryParameters;
type GitContextInput = subgraph_check_query::GitContextInput;

#[derive(Debug, Clone, PartialEq)]
pub struct SubgraphCheckInput {
    pub graph_id: String,
    pub variant: String,
    pub subgraph: String,
    pub proposed_schema: String,
    pub git_context: GitContext,
    pub config: SubgraphCheckConfig,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SubgraphCheckConfig {
    pub query_count_threshold: Option<i64>,
    pub query_count_threshold_percentage: Option<f64>,
    pub validation_period_from: Option<String>,
    pub validation_period_to: Option<String>,
}

impl From<SubgraphCheckInput> for QueryVariables {
    fn from(input: SubgraphCheckInput) -> Self {
        Self {
            graph_id: input.graph_id,
            variant: input.variant,
            subgraph: input.subgraph,
            proposed_schema: QuerySchema {
                sdl: Some(input.proposed_schema),
                hash: None,
            },
            config: QueryConfig {
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
pub struct SubgraphCheckResponse {
    pub target_url: Option<String>,
    pub number_of_checked_operations: i64,
    pub changes: Vec<SchemaChange>,
    pub change_severity: ChangeSeverity,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ChangeSeverity {
    PASS,
    FAIL,
}

impl fmt::Display for ChangeSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let msg = match self {
            ChangeSeverity::PASS => "PASS",
            ChangeSeverity::FAIL => "FAIL",
        };
        write!(f, "{}", msg)
    }
}

impl From<QueryChangeSeverity> for ChangeSeverity {
    fn from(severity: QueryChangeSeverity) -> Self {
        match severity {
            QueryChangeSeverity::NOTICE => ChangeSeverity::PASS,
            QueryChangeSeverity::FAILURE => ChangeSeverity::FAIL,
            _ => unreachable!("Unknown change severity"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SchemaChange {
    pub code: String,
    pub description: String,
    pub severity: ChangeSeverity,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompositionError {
    pub message: String,
    pub code: Option<String>,
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

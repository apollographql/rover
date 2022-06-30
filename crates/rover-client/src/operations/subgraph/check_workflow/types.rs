use crate::operations::subgraph::check_workflow::runner::subgraph_check_workflow_query;
use crate::shared::{ChangeSeverity, CheckConfig, GitContext, GraphRef};

type QueryVariables = subgraph_check_workflow_query::Variables;
pub(crate) type QueryResponseData = subgraph_check_workflow_query::ResponseData;

#[derive(Debug, Clone, PartialEq)]
pub struct CheckWorkflowInput {
    pub graph_ref: GraphRef,
    pub workflow_id: String,
}

impl From<CheckWorkflowInput> for QueryVariables {
    fn from(input: CheckWorkflowInput) -> Self {
        Self {
            graph_id: input.graph_ref.name,
            workflow_id: input.workflow_id,
        }
    }
}

pub(crate) type QueryChangeSeverity = subgraph_check_workflow_query::ChangeSeverity;
impl From<QueryChangeSeverity> for ChangeSeverity {
    fn from(severity: QueryChangeSeverity) -> Self {
        match severity {
            QueryChangeSeverity::NOTICE => ChangeSeverity::PASS,
            QueryChangeSeverity::FAILURE => ChangeSeverity::FAIL,
            _ => ChangeSeverity::unreachable(),
        }
    }
}

pub(crate) type QueryChangeStatus = subgraph_check_workflow_query::CheckWorkflowTaskStatus;
impl From<QueryChangeStatus> for ChangeSeverity {
    fn from(status: QueryChangeStatus) -> Self {
        // we want to re-poll the result if the check is pending or blocked
        // so only consider PASSED as PASS
        match status {
            QueryChangeStatus::PASSED => ChangeSeverity::PASS,
            QueryChangeStatus::FAILED => ChangeSeverity::FAIL,
            QueryChangeStatus::PENDING => ChangeSeverity::FAIL,
            QueryChangeStatus::BLOCKED => ChangeSeverity::FAIL,
            QueryChangeStatus::Other(_) => ChangeSeverity::FAIL,
        }
    }
}







// type MutationVariables = subgraph_check_mutation::Variables;

// pub(crate) type MutationResponseData = subgraph_check_mutation::ResponseData;
// pub(crate) type MutationCompositionErrors =
//     subgraph_check_mutation::SubgraphCheckMutationGraphCheckPartialSchemaCompositionValidationResultErrors;

// type MutationSchema = subgraph_check_mutation::PartialSchemaInput;
// type MutationConfig = subgraph_check_mutation::HistoricQueryParameters;

// pub(crate) type MutationChangeSeverity = subgraph_check_mutation::ChangeSeverity;
// impl From<MutationChangeSeverity> for ChangeSeverity {
//     fn from(severity: MutationChangeSeverity) -> Self {
//         match severity {
//             MutationChangeSeverity::NOTICE => ChangeSeverity::PASS,
//             MutationChangeSeverity::FAILURE => ChangeSeverity::FAIL,
//             _ => ChangeSeverity::unreachable(),
//         }
//     }
// }

// type MutationGitContextInput = subgraph_check_mutation::GitContextInput;
// impl From<GitContext> for MutationGitContextInput {
//     fn from(git_context: GitContext) -> MutationGitContextInput {
//         MutationGitContextInput {
//             branch: git_context.branch,
//             commit: git_context.commit,
//             committer: git_context.author,
//             remoteUrl: git_context.remote_url,
//             message: None,
//         }
//     }
// }

// #[derive(Debug, Clone, PartialEq)]
// pub struct SubgraphCheckInput {
//     pub graph_ref: GraphRef,
//     pub subgraph: String,
//     pub proposed_schema: String,
//     pub git_context: GitContext,
//     pub config: CheckConfig,
// }

// impl From<SubgraphCheckInput> for MutationVariables {
//     fn from(input: SubgraphCheckInput) -> Self {
//         Self {
//             graph_id: input.graph_ref.name,
//             variant: input.graph_ref.variant,
//             subgraph: input.subgraph,
//             proposed_schema: MutationSchema {
//                 sdl: Some(input.proposed_schema),
//                 hash: None,
//             },
//             config: input.config.into(),
//             git_context: input.git_context.into(),
//         }
//     }

// impl From<CheckConfig> for MutationConfig {
//     fn from(input: CheckConfig) -> Self {
//         let (from, to) = match input.validation_period {
//             Some(validation_period) => (
//                 Some(validation_period.from.to_string()),
//                 Some(validation_period.to.to_string()),
//             ),
//             None => (None, None),
//         };
//         Self {
//             queryCountThreshold: input.query_count_threshold,
//             queryCountThresholdPercentage: input.query_count_threshold_percentage,
//             from,
//             to,
//             // we don't support configuring these, but we can't leave them out
//             excludedClients: None,
//             excludedOperationNames: None,
//             ignoredOperations: None,
//             includedVariants: None,
//         }
//     }
// }

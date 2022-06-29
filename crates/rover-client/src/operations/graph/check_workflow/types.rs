use crate::operations::graph::check_workflow::runner::graph_check_workflow_query;
use crate::shared::{ChangeSeverity, GraphRef};

type QueryVariables = graph_check_workflow_query::Variables;
pub(crate) type QueryResponseData = graph_check_workflow_query::ResponseData;

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

pub(crate) type MutationChangeSeverity = graph_check_workflow_query::ChangeSeverity;
impl From<MutationChangeSeverity> for ChangeSeverity {
    fn from(severity: MutationChangeSeverity) -> Self {
        match severity {
            MutationChangeSeverity::NOTICE => ChangeSeverity::PASS,
            MutationChangeSeverity::FAILURE => ChangeSeverity::FAIL,
            _ => ChangeSeverity::unreachable(),
        }
    }
}

pub(crate) type MutationChangeStatus = graph_check_workflow_query::CheckWorkflowTaskStatus;
impl From<MutationChangeStatus> for ChangeSeverity {
    fn from(status: MutationChangeStatus) -> Self {
        // we want to re-poll the result if the check is pending or blocked
        // so only consider PASSED as PASS
        match status {
            MutationChangeStatus::PASSED => ChangeSeverity::PASS,
            MutationChangeStatus::FAILED => ChangeSeverity::FAIL,
            MutationChangeStatus::PENDING => ChangeSeverity::FAIL,
            MutationChangeStatus::BLOCKED => ChangeSeverity::FAIL,
            MutationChangeStatus::Other(_) => ChangeSeverity::FAIL,
        }
    }
}
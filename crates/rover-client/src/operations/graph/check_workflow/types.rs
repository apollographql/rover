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

pub(crate) type QueryChangeSeverity = graph_check_workflow_query::ChangeSeverity;
impl From<QueryChangeSeverity> for ChangeSeverity {
    fn from(severity: QueryChangeSeverity) -> Self {
        match severity {
            QueryChangeSeverity::NOTICE => ChangeSeverity::PASS,
            QueryChangeSeverity::FAILURE => ChangeSeverity::FAIL,
            _ => ChangeSeverity::unreachable(),
        }
    }
}

pub(crate) type QueryChangeStatus = graph_check_workflow_query::CheckWorkflowTaskStatus;
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

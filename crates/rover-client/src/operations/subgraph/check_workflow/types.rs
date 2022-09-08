use crate::operations::subgraph::check_workflow::runner::subgraph_check_workflow_query;
use crate::shared::{ChangeSeverity, GraphRef};

type QueryVariables = subgraph_check_workflow_query::Variables;
pub(crate) type QueryResponseData = subgraph_check_workflow_query::ResponseData;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct CheckWorkflowInput {
    pub graph_ref: GraphRef,
    pub workflow_id: String,
    pub checks_timeout_seconds: u64,
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

pub(crate) type WorkflowStatus = subgraph_check_workflow_query::CheckWorkflowStatus;
impl From<WorkflowStatus> for ChangeSeverity {
    fn from(status: WorkflowStatus) -> Self {
        // we want to re-poll the result if the check is pending or blocked
        // so only consider PASSED as PASS
        match status {
            WorkflowStatus::PASSED => ChangeSeverity::PASS,
            WorkflowStatus::FAILED => ChangeSeverity::FAIL,
            WorkflowStatus::PENDING => ChangeSeverity::FAIL,
            WorkflowStatus::Other(_) => ChangeSeverity::FAIL,
        }
    }
}

use crate::operations::graph::check::runner::graph_check_workflow_query;
use crate::shared::{ChangeSeverity, CheckConfig, GitContext, GraphRef, SchemaChange};

type QueryVariables = graph_check_workflow_query::Variables;
pub(crate) type QueryResponseData = graph_check_workflow_query::ResponseData;

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

use std::str::FromStr;

use crate::blocking::StudioClient;
use crate::operations::workflow::status::types::{
  CheckWorkflowInput,
  CheckWorkflowResponse,
  QueryResponseData,
  CheckWorkflowStatus,
  CheckWorkflowTaskStatus,
  CompositionResult,
  OperationCheckResult,
  ChangeSeverity,
  CheckWorkflowTask,
  SchemaCompositionError
};
use crate::RoverClientError;
use crate::shared::{GraphRef, GitContext};
use crate::operations::workflow::status::runner::check_workflow_query::CheckWorkflowQueryGraphCheckWorkflowTasksOn::{CompositionCheckTask, OperationsCheckTask};
use graphql_client::*;

type Timestamp = String;
#[derive(GraphQLQuery)]
// The paths are relative to the directory where your `Cargo.toml` is located.
// Both json and the GraphQL schema language are supported as sources for the schema
#[graphql(
    query_path = "src/operations/workflow/status/check_workflow_query.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn",
    extern_enums("CheckWorkflowStatus", "CheckWorkflowTaskStatus", "ChangeSeverity"),
)]
/// This struct is used to generate the module containing `Variables` and
/// `ResponseData` structs.
/// Snake case of this name is the mod name. i.e. graph_check_mutation
pub(crate) struct CheckWorkflowQuery;

/// The main function to be used from this module.
/// This function takes a workflow id and returns the status of the workflow.
pub fn run(
  input: CheckWorkflowInput,
  client: &StudioClient,
) -> Result<CheckWorkflowResponse, RoverClientError> {
  let graph_ref = input.graph_ref.clone();
  let response_data = client.post::<CheckWorkflowQuery>(input.into())?;
  get_workflow_status_from_response_data(response_data, graph_ref)
}

fn get_workflow_status_from_response_data(
  data: QueryResponseData,
  graph_ref: GraphRef,
) -> Result<CheckWorkflowResponse, RoverClientError> {
  let graph = data.graph.ok_or(RoverClientError::GraphNotFound {
    graph_ref: graph_ref.clone(),
  })?;
  let check_workflow = graph.check_workflow.ok_or(RoverClientError::GraphNotFound {
    graph_ref: graph_ref.clone(),
  })?;

  let mut check_tasks = Vec::new();
  for task in check_workflow.tasks {
    let mut composition_result: Option<CompositionResult> = None;
    let mut operation_check_result: Option<OperationCheckResult> = None;
    match task.on {
      CompositionCheckTask(task) => {
        if let Some(result) = task.result {
          composition_result = Some(CompositionResult {
            graph_composition_id: result.graph_composition_id,
            errors: result.errors.iter().map(|e| SchemaCompositionError{message: e.message.clone()}).collect(),
          })
        }
      },
      OperationsCheckTask(task) => {
        if let Some(result) = task.result {
          operation_check_result = Some(OperationCheckResult {
            id: result.id,
            check_severity: result.check_severity,
            number_of_checked_operations: result.number_of_checked_operations,
            number_of_affected_operations: result.number_of_affected_operations,
            created_at: result.created_at,
          })
        }
      },
    }
    let task = CheckWorkflowTask {
      id: task.id,
      status: task.status,
      created_at: task.created_at,
      completed_at: task.completed_at,
      composition_result: composition_result,
      operation_check_result: operation_check_result,
    };
    check_tasks.push(task);
  }

  Ok(CheckWorkflowResponse {
    base_variant:check_workflow.base_variant.and_then(|v| GraphRef::from_str(&v.id).ok()),
    git_context: Some(GitContext {
      branch: None,
      author: None,
      commit: check_workflow.git_context.and_then(|g| g.commit),
      remote_url: None,
    }),
    implementing_service_name: check_workflow.implementing_service_name,
    completed_at: check_workflow.completed_at,
    created_at: check_workflow.created_at,
    started_at: check_workflow.started_at,
    status: check_workflow.status,
    tasks: check_tasks
  })
}
use std::time::{Duration, Instant};

use crate::blocking::StudioClient;
use crate::operations::graph::check_workflow::types::{CheckWorkflowInput, QueryResponseData};
use crate::shared::{CheckResponse, GraphRef, SchemaChange};
use crate::RoverClientError;

use graphql_client::*;

use self::graph_check_workflow_query::GraphCheckWorkflowQueryGraphCheckWorkflowTasks::OperationsCheckTask;
use self::graph_check_workflow_query::{CheckWorkflowStatus, CheckWorkflowTaskStatus};

use super::types::OperationsResult;

#[derive(GraphQLQuery)]
// The paths are relative to the directory where your `Cargo.toml` is located.
// Both json and the GraphQL schema language are supported as sources for the schema
#[graphql(
    query_path = "src/operations/graph/check_workflow/check_workflow_query.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "Eq, PartialEq, Debug, Serialize, Deserialize, Clone",
    deprecated = "warn"
)]
/// This struct is used to generate the module containing `Variables` and
/// `ResponseData` structs.
/// Snake case of this name is the mod name. i.e. graph_check_workflow_query
pub(crate) struct GraphCheckWorkflowQuery;

/// The main function to be used from this module.
/// This function takes a proposed schema and validates it against a published
/// schema.
pub fn run(
    input: CheckWorkflowInput,
    client: &StudioClient,
) -> Result<CheckResponse, RoverClientError> {
    let graph_ref = input.graph_ref.clone();
    let mut data;
    let now = Instant::now();
    loop {
        data = client.post::<GraphCheckWorkflowQuery>(input.clone().into())?;
        let graph = data.clone().graph.ok_or(RoverClientError::GraphNotFound {
            graph_ref: graph_ref.clone(),
        })?;
        if let Some(check_workflow) = graph.check_workflow {
            if !matches!(check_workflow.status, CheckWorkflowStatus::PENDING) {
                break;
            }
        }
        if now.elapsed() > Duration::from_secs(input.checks_timeout_seconds) {
            return Err(RoverClientError::ChecksTimeoutError {
                url: get_target_url_from_data(data),
            });
        }
        std::thread::sleep(Duration::from_secs(5));
    }
    get_check_response_from_data(data, graph_ref)
}

fn get_check_response_from_data(
    data: QueryResponseData,
    graph_ref: GraphRef,
) -> Result<CheckResponse, RoverClientError> {
    let graph = data.graph.ok_or(RoverClientError::GraphNotFound {
        graph_ref: graph_ref.clone(),
    })?;
    let check_workflow = graph
        .check_workflow
        .ok_or(RoverClientError::GraphNotFound {
            graph_ref: graph_ref.clone(),
        })?;

    let workflow_status = check_workflow.status;
    let mut operations_status = None;
    let mut operations_target_url = None;
    let mut operations_result: Option<OperationsResult> = None;
    let mut number_of_checked_operations: u64 = 0;
    for task in check_workflow.tasks {
        if let OperationsCheckTask(task) = task {
            operations_status = Some(task.status);
            operations_target_url = task.target_url;
            if let Some(result) = task.result {
                number_of_checked_operations =
                    result.number_of_checked_operations.try_into().unwrap();
                operations_result = Some(result);
            }
        }
    }

    if matches!(operations_status, Some(CheckWorkflowTaskStatus::FAILED))
        || matches!(workflow_status, CheckWorkflowStatus::PASSED)
    {
        let result = operations_result.ok_or(RoverClientError::MalformedResponse {
            null_field: "OperationsCheckTask.result".to_string(),
        })?;
        let mut changes = Vec::with_capacity(result.changes.len());
        for change in result.changes {
            changes.push(SchemaChange {
                code: change.code,
                severity: change.severity.into(),
                description: change.description,
            });
        }

        // The `graph` check response does not return this field
        // only `subgraph` check does. Since `CheckResponse` is shared
        // between `graph` and `subgraph` checks, defaulting this
        // to false for now since its currently only used in
        // `check_response.rs` to format better console messages.
        let core_schema_modified = false;

        CheckResponse::try_new(
            operations_target_url,
            number_of_checked_operations,
            changes,
            workflow_status.into(),
            graph_ref,
            core_schema_modified,
        )
    } else {
        // Note that graph IDs and variants don't need percent-encoding due to their regex restrictions.
        let default_target_url = format!(
            "https://studio.apollographql.com/graph/{}/checks?variant={}",
            graph_ref.name, graph_ref.variant
        );
        Err(RoverClientError::OtherCheckTaskFailure {
            has_build_task: false,
            has_downstream_task: false,
            target_url: operations_target_url.unwrap_or(default_target_url),
        })
    }
}

fn get_target_url_from_data(data: QueryResponseData) -> Option<String> {
    let mut target_url = None;
    if let Some(graph) = data.graph {
        if let Some(check_workflow) = graph.check_workflow {
            for task in check_workflow.tasks {
                if let OperationsCheckTask(task) = task {
                    target_url = task.target_url;
                }
            }
        }
    }
    target_url
}

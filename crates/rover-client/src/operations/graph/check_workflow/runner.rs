use std::time::{Duration, Instant};

use graphql_client::*;

use crate::blocking::StudioClient;
use crate::operations::graph::check_workflow::types::{CheckWorkflowInput, QueryResponseData};
use crate::shared::{
    CheckWorkflowResponse, CustomCheckResponse, Diagnostic, GraphRef, LintCheckResponse,
    OperationCheckResponse, SchemaChange, Violation,
};
use crate::RoverClientError;

use self::graph_check_workflow_query::GraphCheckWorkflowQueryGraphCheckWorkflowTasksOn::{
    CustomCheckTask, LintCheckTask, OperationsCheckTask,
};
use self::graph_check_workflow_query::{
    CheckWorkflowStatus, CheckWorkflowTaskStatus,
    GraphCheckWorkflowQueryGraphCheckWorkflowTasksOnCustomCheckTaskResult,
    GraphCheckWorkflowQueryGraphCheckWorkflowTasksOnLintCheckTaskResult,
    GraphCheckWorkflowQueryGraphCheckWorkflowTasksOnOperationsCheckTaskResult,
};

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
pub async fn run(
    input: CheckWorkflowInput,
    client: &StudioClient,
) -> Result<CheckWorkflowResponse, RoverClientError> {
    let graph_ref = input.graph_ref.clone();
    let mut url: Option<String> = None;
    let now = Instant::now();
    loop {
        let result = client
            .post::<GraphCheckWorkflowQuery>(input.clone().into())
            .await;
        match result {
            Ok(data) => {
                let graph = data.clone().graph.ok_or(RoverClientError::GraphNotFound {
                    graph_ref: graph_ref.clone(),
                })?;
                if let Some(check_workflow) = graph.check_workflow {
                    if !matches!(check_workflow.status, CheckWorkflowStatus::PENDING) {
                        return get_check_response_from_data(data, graph_ref);
                    }
                }
                url = get_target_url_from_data(data);
            }
            Err(e) => {
                eprintln!("error while checking status of check: {e}\nthis error may be transient... retrying");
            }
        }
        if now.elapsed() > Duration::from_secs(input.checks_timeout_seconds) {
            return Err(RoverClientError::ChecksTimeoutError { url });
        }
        std::thread::sleep(Duration::from_secs(5));
    }
}

fn get_check_response_from_data(
    data: QueryResponseData,
    graph_ref: GraphRef,
) -> Result<CheckWorkflowResponse, RoverClientError> {
    let graph = data.graph.ok_or(RoverClientError::GraphNotFound {
        graph_ref: graph_ref.clone(),
    })?;
    let check_workflow = graph
        .check_workflow
        .ok_or(RoverClientError::GraphNotFound {
            graph_ref: graph_ref.clone(),
        })?;

    let mut operations_status = None;
    let mut operations_target_url = None;
    let mut operations_result: Option<
        GraphCheckWorkflowQueryGraphCheckWorkflowTasksOnOperationsCheckTaskResult,
    > = None;
    let mut number_of_checked_operations: u64 = 0;

    let mut lint_status = None;
    let mut lint_target_url = None;
    let mut lint_result: Option<
        GraphCheckWorkflowQueryGraphCheckWorkflowTasksOnLintCheckTaskResult,
    > = None;

    let mut custom_status = None;
    let mut custom_target_url = None;
    let mut custom_result: Option<
        GraphCheckWorkflowQueryGraphCheckWorkflowTasksOnCustomCheckTaskResult,
    > = None;

    for task in check_workflow.tasks {
        match task.on {
            OperationsCheckTask(typed_task) => {
                operations_status = Some(task.status);
                operations_target_url = task.target_url;
                if let Some(result) = typed_task.result {
                    number_of_checked_operations =
                        result.number_of_checked_operations.try_into().unwrap();
                    operations_result = Some(result);
                }
            }
            LintCheckTask(typed_task) => {
                lint_status = Some(task.status);
                lint_target_url = task.target_url;
                if let Some(result) = typed_task.result {
                    lint_result = Some(result)
                }
            }
            CustomCheckTask(typed_task) => {
                custom_status = Some(task.status);
                custom_target_url = task.target_url;
                if let Some(result) = typed_task.result {
                    custom_result = Some(result)
                }
            }
            _ => (),
        }
    }

    // Note that graph IDs and variants don't need percent-encoding due to their regex restrictions.
    let default_target_url = format!(
        "https://studio.apollographql.com/graph/{}/checks?variant={}",
        graph_ref.name, graph_ref.variant
    );

    let check_response = CheckWorkflowResponse {
        default_target_url: default_target_url.clone(),
        maybe_core_schema_modified: None,
        maybe_operations_response: get_operations_response_from_result(
            operations_target_url,
            number_of_checked_operations,
            operations_status.unwrap_or(CheckWorkflowTaskStatus::PENDING),
            operations_result,
        ),
        maybe_lint_response: get_lint_response_from_result(
            lint_status,
            lint_target_url,
            lint_result,
        ),
        maybe_custom_response: get_custom_response_from_result(
            custom_status,
            custom_target_url,
            custom_result,
        ),
        maybe_proposals_response: None,
        maybe_downstream_response: None,
    };

    match check_workflow.status {
        CheckWorkflowStatus::PASSED => Ok(check_response),
        CheckWorkflowStatus::FAILED => Err(RoverClientError::CheckWorkflowFailure {
            graph_ref,
            check_response: Box::new(check_response),
        }),
        _ => Err(RoverClientError::UnknownCheckWorkflowStatus),
    }
}

fn get_target_url_from_data(data: QueryResponseData) -> Option<String> {
    let mut target_url = None;
    if let Some(graph) = data.graph {
        if let Some(check_workflow) = graph.check_workflow {
            for task in check_workflow.tasks {
                match task.on {
                    OperationsCheckTask(_) => target_url = task.target_url,
                    LintCheckTask(_) => target_url = task.target_url,
                    CustomCheckTask(_) => target_url = task.target_url,
                    _ => (),
                }
            }
        }
    }
    target_url
}

fn get_operations_response_from_result(
    target_url: Option<String>,
    number_of_checked_operations: u64,
    task_status: CheckWorkflowTaskStatus,
    results: Option<GraphCheckWorkflowQueryGraphCheckWorkflowTasksOnOperationsCheckTaskResult>,
) -> Option<OperationCheckResponse> {
    match results {
        Some(result) => {
            let mut changes = Vec::with_capacity(result.changes.len());
            for change in result.changes {
                changes.push(SchemaChange {
                    code: change.code,
                    severity: change.severity.into(),
                    description: change.description,
                });
            }
            Some(OperationCheckResponse::try_new(
                Some(task_status).into(),
                target_url,
                number_of_checked_operations,
                changes,
            ))
        }
        None => None,
    }
}

fn get_lint_response_from_result(
    task_status: Option<CheckWorkflowTaskStatus>,
    target_url: Option<String>,
    results: Option<GraphCheckWorkflowQueryGraphCheckWorkflowTasksOnLintCheckTaskResult>,
) -> Option<LintCheckResponse> {
    match results {
        Some(result) => {
            let mut diagnostics = Vec::with_capacity(result.diagnostics.len());
            for diagnostic in result.diagnostics {
                let mut start_line = 0;
                let mut start_byte_offset = 0;
                let mut end_byte_offset = 0;
                // loc 0 is graph and 1 is subgraph
                if let Some(start) = &diagnostic.source_locations[0].start {
                    start_line = start.line;
                    start_byte_offset = start.byte_offset;
                }
                if let Some(end) = &diagnostic.source_locations[0].end {
                    end_byte_offset = end.byte_offset;
                }
                diagnostics.push(Diagnostic {
                    level: diagnostic.level.to_string(),
                    message: diagnostic.message,
                    coordinate: diagnostic.coordinate,
                    rule: diagnostic.rule.to_string(),
                    start_line,
                    start_byte_offset: start_byte_offset.unsigned_abs() as usize,
                    end_byte_offset: end_byte_offset.unsigned_abs() as usize,
                })
            }
            Some(LintCheckResponse {
                task_status: task_status.into(),
                target_url,
                diagnostics,
                errors_count: result.stats.errors_count.unsigned_abs(),
                warnings_count: result.stats.warnings_count.unsigned_abs(),
            })
        }
        None => None,
    }
}

fn get_custom_response_from_result(
    task_status: Option<CheckWorkflowTaskStatus>,
    target_url: Option<String>,
    results: Option<GraphCheckWorkflowQueryGraphCheckWorkflowTasksOnCustomCheckTaskResult>,
) -> Option<CustomCheckResponse> {
    match results {
        Some(result) => {
            let violations: Vec<Violation> = result.violations.iter().map(|violation| {
                let start_line = if let Some(source_locations) = &violation.source_locations {
                    if !source_locations.is_empty() {
                        Some(source_locations[0].start.line)
                    } else {
                        None
                    }
                } else {
                    None
                };
                Violation {
                    level: violation.level.to_string(),
                    message: violation.message.clone(),
                    start_line,
                    rule: violation.rule.clone(),
                }
            }).collect();
            Some(CustomCheckResponse {
                task_status: task_status.into(),
                target_url,
                violations,
            })
        }
        None => None,
    }
}

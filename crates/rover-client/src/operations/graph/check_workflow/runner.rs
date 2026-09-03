use graphql_client::*;
use rover_studio::types::GraphRef;

use self::graph_check_workflow_query::{
    CheckWorkflowStatus, CheckWorkflowTaskStatus,
    GraphCheckWorkflowQueryGraphCheckWorkflowTasksOn::{
        CustomCheckTask, DownstreamCheckTask, LintCheckTask, OperationsCheckTask,
    },
    GraphCheckWorkflowQueryGraphCheckWorkflowTasksOnCustomCheckTaskResult,
    GraphCheckWorkflowQueryGraphCheckWorkflowTasksOnDownstreamCheckTaskResults,
    GraphCheckWorkflowQueryGraphCheckWorkflowTasksOnLintCheckTaskResult,
    GraphCheckWorkflowQueryGraphCheckWorkflowTasksOnOperationsCheckTaskResult,
};
use crate::{
    blocking::StudioClient,
    operations::graph::check_workflow::types::{CheckWorkflowInput, QueryResponseData},
    shared::{
        check_workflow_poll::{poll_check_workflow, PollState},
        CheckTaskStatus, CheckWorkflowResponse, CustomCheckResponse, Diagnostic,
        DownstreamCheckResponse, DownstreamVariantCheckResult, LintCheckResponse,
        OperationCheckResponse, SchemaChange, Violation,
    },
    RoverClientError,
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

#[derive(GraphQLQuery)]
#[graphql(
    query_path = "src/operations/graph/check_workflow/check_workflow_status_query.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "Eq, PartialEq, Debug, Serialize, Deserialize, Clone",
    deprecated = "warn"
)]
/// A lightweight, status-only poll query. Used to wait for the check workflow to
/// finish without re-fetching the (potentially huge) per-task results on every
/// poll — the full result is fetched once, after the workflow leaves PENDING.
pub(crate) struct GraphCheckWorkflowStatusQuery;

/// The main function to be used from this module.
/// This function takes a proposed schema and validates it against a published
/// schema.
pub async fn run(
    input: CheckWorkflowInput,
    client: &StudioClient,
) -> Result<CheckWorkflowResponse, RoverClientError> {
    let graph_ref = input.graph_ref.clone();
    let checks_timeout_seconds = input.checks_timeout_seconds;

    let data = poll_check_workflow(
        checks_timeout_seconds,
        async || {
            let data = client
                .post::<GraphCheckWorkflowStatusQuery>(input.clone().into())
                .await?;
            // The graph (and its check workflow) may not be reportable on the very
            // first polls; treat that as "not ready yet" and keep polling.
            let Some(check_workflow) = data.graph.and_then(|graph| graph.check_workflow) else {
                return Ok(None);
            };
            Ok(Some(PollState {
                finished: !matches!(
                    check_workflow.status,
                    graph_check_workflow_status_query::CheckWorkflowStatus::PENDING
                ),
                target_url: get_target_url_from_status_data(&check_workflow),
            }))
        },
        async || {
            client
                .post::<GraphCheckWorkflowQuery>(input.clone().into())
                .await
        },
    )
    .await?;
    get_check_response_from_data(data, graph_ref)
}

fn get_target_url_from_status_data(
    check_workflow: &graph_check_workflow_status_query::GraphCheckWorkflowStatusQueryGraphCheckWorkflow,
) -> Option<String> {
    check_workflow
        .tasks
        .iter()
        .filter_map(|task| task.target_url.clone())
        .next_back()
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

    let mut downstream_status = None;
    let mut downstream_target_url = None;
    let mut downstream_result: Option<
        Vec<GraphCheckWorkflowQueryGraphCheckWorkflowTasksOnDownstreamCheckTaskResults>,
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
            DownstreamCheckTask(typed_task) => {
                downstream_status = Some(task.status);
                downstream_target_url = task.target_url;
                if let Some(results) = typed_task.results {
                    downstream_result = Some(results)
                }
            }
            _ => (),
        }
    }

    // Note that graph IDs and variants don't need percent-encoding due to their regex restrictions.
    let default_target_url = format!(
        "https://studio.apollographql.com/graph/{}/checks?variant={}",
        graph_ref.graph_id(),
        graph_ref.variant()
    );

    let maybe_downstream_response = get_downstream_response_from_result(
        downstream_status,
        downstream_target_url,
        downstream_result,
    );
    let downstream_failed = maybe_downstream_response
        .as_ref()
        .map(|response| response.task_status == CheckTaskStatus::FAILED)
        .unwrap_or(false);

    let check_response = CheckWorkflowResponse {
        default_target_url,
        maybe_core_schema_modified: None,
        maybe_core_schema_status: None,
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
        maybe_downstream_response,
    };

    match check_workflow.status {
        CheckWorkflowStatus::PASSED if !downstream_failed => Ok(check_response),
        CheckWorkflowStatus::FAILED => Err(RoverClientError::CheckWorkflowFailure {
            graph_ref,
            check_response: Box::new(check_response),
        }),
        CheckWorkflowStatus::PASSED => Err(RoverClientError::CheckWorkflowFailure {
            graph_ref,
            check_response: Box::new(check_response),
        }),
        _ => Err(RoverClientError::UnknownCheckWorkflowStatus),
    }
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
            let violations: Vec<Violation> = result
                .violations
                .iter()
                .map(|violation| {
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
                })
                .collect();
            Some(CustomCheckResponse {
                task_status: task_status.into(),
                target_url,
                violations,
            })
        }
        None => None,
    }
}

fn get_downstream_response_from_result(
    task_status: Option<CheckWorkflowTaskStatus>,
    target_url: Option<String>,
    results: Option<
        Vec<GraphCheckWorkflowQueryGraphCheckWorkflowTasksOnDownstreamCheckTaskResults>,
    >,
) -> Option<DownstreamCheckResponse> {
    match results {
        Some(results) => {
            let variants: Vec<DownstreamVariantCheckResult> = results
                .into_iter()
                .map(|result| DownstreamVariantCheckResult {
                    graph_id: result.downstream_graph_id,
                    variant_name: result.downstream_variant_name,
                    blocking: result.blocking,
                    fails_upstream_workflow: result.fails_upstream_workflow,
                    status: match result.downstream_workflow.map(|workflow| workflow.status) {
                        Some(CheckWorkflowStatus::FAILED) => CheckTaskStatus::FAILED,
                        Some(CheckWorkflowStatus::PASSED) => CheckTaskStatus::PASSED,
                        Some(CheckWorkflowStatus::PENDING) => CheckTaskStatus::PENDING,
                        // Not yet initialized, or the downstream variant was deleted.
                        None => CheckTaskStatus::PENDING,
                        _ => CheckTaskStatus::FAILED,
                    },
                })
                .collect();
            // A blocking downstream contract workflow that has actually failed makes this
            // task FAILED even if the task's own aggregate status hasn't caught up yet.
            let has_blocking_failure = variants.iter().any(|variant| {
                variant.fails_upstream_workflow.unwrap_or(false)
                    || (variant.blocking && variant.status == CheckTaskStatus::FAILED)
            });
            let task_status = if has_blocking_failure {
                CheckTaskStatus::FAILED
            } else {
                task_status.into()
            };
            Some(DownstreamCheckResponse {
                task_status,
                target_url,
                variants,
            })
        }
        None => None,
    }
}

#[cfg(test)]
mod tests {
    use rover_studio::types::GraphRef;
    use rstest::rstest;
    use serde_json::{json, Value};
    use speculoos::prelude::*;

    use super::*;
    use crate::operations::graph::check_workflow::types::QueryResponseData;

    fn create_check_workflow_data(
        status: CheckWorkflowStatus,
        tasks: serde_json::Value,
    ) -> QueryResponseData {
        serde_json::from_value(json!({
            "graph": {
                "checkWorkflow": {
                    "id": "test-workflow",
                    "status": status,
                    "tasks": tasks
                }
            }
        }))
        .unwrap()
    }

    #[test]
    fn no_contract_variants_configured_still_passes() {
        let data = create_check_workflow_data(
            CheckWorkflowStatus::PASSED,
            json!([
                {
                    "__typename": "DownstreamCheckTask",
                    "id": "downstream-task",
                    "status": "PASSED",
                    "targetUrl": "https://studio.apollographql.com/graph/test-graph/checks/downstream",
                    "results": []
                }
            ]),
        );
        let graph_ref: GraphRef = "test-graph@test-variant".parse().unwrap();

        let response = get_check_response_from_data(data, graph_ref).unwrap();

        let downstream = response
            .maybe_downstream_response
            .expect("expected downstream response");
        assert_that!(&downstream.task_status).is_equal_to(CheckTaskStatus::PASSED);
        assert_that!(&downstream.variants).is_empty();
    }

    #[test]
    fn all_contract_variants_passed() {
        let data = create_check_workflow_data(
            CheckWorkflowStatus::PASSED,
            json!([
                {
                    "__typename": "DownstreamCheckTask",
                    "id": "downstream-task",
                    "status": "PASSED",
                    "targetUrl": "https://studio.apollographql.com/graph/test-graph/checks/downstream",
                    "results": [
                        {
                            "__typename": "DownstreamCheckResult",
                            "blocking": true,
                            "downstreamGraphID": "test-graph",
                            "downstreamVariantName": "mobile",
                            "downstreamWorkflow": { "status": "PASSED" },
                            "failsUpstreamWorkflow": false
                        },
                        {
                            "__typename": "DownstreamCheckResult",
                            "blocking": true,
                            "downstreamGraphID": "test-graph",
                            "downstreamVariantName": "partner-api",
                            "downstreamWorkflow": { "status": "PASSED" },
                            "failsUpstreamWorkflow": false
                        }
                    ]
                }
            ]),
        );
        let graph_ref: GraphRef = "test-graph@test-variant".parse().unwrap();

        let result = get_check_response_from_data(data, graph_ref);

        assert_that!(&result).is_ok();
        let downstream = result
            .unwrap()
            .maybe_downstream_response
            .expect("expected downstream response");
        assert_that!(&downstream.task_status).is_equal_to(CheckTaskStatus::PASSED);
        assert_that!(&downstream.variants).has_length(2);
    }

    #[test]
    fn blocking_downstream_workflow_failure_fails_the_check() {
        let data = create_check_workflow_data(
            CheckWorkflowStatus::PASSED,
            json!([
                {
                    "__typename": "DownstreamCheckTask",
                    "id": "downstream-task",
                    "status": "PASSED",
                    "targetUrl": "https://studio.apollographql.com/graph/test-graph/checks/downstream",
                    "results": [
                        {
                            "__typename": "DownstreamCheckResult",
                            "blocking": true,
                            "downstreamGraphID": "test-graph",
                            "downstreamVariantName": "mobile",
                            "downstreamWorkflow": { "status": "FAILED" },
                            "failsUpstreamWorkflow": null
                        }
                    ]
                }
            ]),
        );
        let graph_ref: GraphRef = "test-graph@test-variant".parse().unwrap();

        let result = get_check_response_from_data(data, graph_ref.clone());

        assert_that!(&result).is_err();
        match result.unwrap_err() {
            RoverClientError::CheckWorkflowFailure {
                graph_ref: returned_graph_ref,
                check_response,
            } => {
                assert_that!(&returned_graph_ref).is_equal_to(&graph_ref);
                let downstream = check_response
                    .maybe_downstream_response
                    .expect("expected downstream response");
                assert_that!(&downstream.task_status).is_equal_to(CheckTaskStatus::FAILED);
            }
            other => panic!("Expected CheckWorkflowFailure error, got {other:?}"),
        }
    }

    #[test]
    fn downstream_task_not_yet_initialized_has_no_response() {
        let data = create_check_workflow_data(
            CheckWorkflowStatus::PASSED,
            json!([
                {
                    "__typename": "DownstreamCheckTask",
                    "id": "downstream-task",
                    "status": "PENDING",
                    "targetUrl": null,
                    "results": null
                }
            ]),
        );
        let graph_ref: GraphRef = "test-graph@test-variant".parse().unwrap();

        let response = get_check_response_from_data(data, graph_ref).unwrap();

        assert_that!(&response.maybe_downstream_response).is_none();
    }

    #[rstest]
    #[case::failed(Some("FAILED"), CheckTaskStatus::FAILED)]
    #[case::passed(Some("PASSED"), CheckTaskStatus::PASSED)]
    #[case::pending(Some("PENDING"), CheckTaskStatus::PENDING)]
    #[case::not_yet_initialized(None, CheckTaskStatus::PENDING)]
    fn downstream_variant_status_mapping(
        #[case] downstream_workflow_status: Option<&str>,
        #[case] expected_status: CheckTaskStatus,
    ) {
        let downstream_workflow = match downstream_workflow_status {
            Some(status) => json!({ "status": status }),
            None => Value::Null,
        };
        let data = create_check_workflow_data(
            CheckWorkflowStatus::PASSED,
            json!([
                {
                    "__typename": "DownstreamCheckTask",
                    "id": "downstream-task",
                    "status": "PASSED",
                    "targetUrl": null,
                    "results": [
                        {
                            "__typename": "DownstreamCheckResult",
                            // Non-blocking, so a FAILED case here doesn't also trip
                            // the fail-the-check behavior covered by the test above.
                            "blocking": false,
                            "downstreamGraphID": "test-graph",
                            "downstreamVariantName": "mobile",
                            "downstreamWorkflow": downstream_workflow,
                            "failsUpstreamWorkflow": null
                        }
                    ]
                }
            ]),
        );
        let graph_ref: GraphRef = "test-graph@test-variant".parse().unwrap();

        let response = get_check_response_from_data(data, graph_ref).unwrap();

        let downstream = response
            .maybe_downstream_response
            .expect("expected downstream response");
        assert_that!(&downstream.variants[0].status).is_equal_to(expected_status);
    }
}

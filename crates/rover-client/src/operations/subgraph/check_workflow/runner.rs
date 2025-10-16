use std::time::{Duration, Instant};

use apollo_federation_types::rover::BuildError;
use graphql_client::*;

use crate::blocking::StudioClient;
use crate::operations::subgraph::check_workflow::types::QueryResponseData;
use crate::shared::{
    CheckWorkflowResponse, CustomCheckResponse, Diagnostic, DownstreamCheckResponse, GraphRef,
    LintCheckResponse, OperationCheckResponse, ProposalsCheckResponse, ProposalsCheckSeverityLevel,
    ProposalsCoverage, RelatedProposal, SchemaChange, Violation,
};
use crate::RoverClientError;

use super::types::*;

use self::subgraph_check_workflow_query::SubgraphCheckWorkflowQueryGraphCheckWorkflowTasksOn::{
    CompositionCheckTask, CustomCheckTask, DownstreamCheckTask, LintCheckTask, OperationsCheckTask,
    ProposalsCheckTask,
};
use self::subgraph_check_workflow_query::{
    CheckWorkflowStatus, CheckWorkflowTaskStatus, ProposalStatus,
    SubgraphCheckWorkflowQueryGraphCheckWorkflowTasksOnCustomCheckTaskResult,
    SubgraphCheckWorkflowQueryGraphCheckWorkflowTasksOnDownstreamCheckTaskResults,
    SubgraphCheckWorkflowQueryGraphCheckWorkflowTasksOnLintCheckTaskResult,
    SubgraphCheckWorkflowQueryGraphCheckWorkflowTasksOnOperationsCheckTaskResult,
};

#[derive(GraphQLQuery)]
// The paths are relative to the directory where your `Cargo.toml` is located.
// Both json and the GraphQL schema language are supported as sources for the schema
#[graphql(
    query_path = "src/operations/subgraph/check_workflow/check_workflow_query.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "PartialEq, Eq, Debug, Serialize, Deserialize, Clone",
    deprecated = "warn"
)]
/// This struct is used to generate the module containing `Variables` and
/// `ResponseData` structs.
/// Snake case of this name is the mod name. i.e. subgraph_check_workflow_query
pub(crate) struct SubgraphCheckWorkflowQuery;

/// The main function to be used from this module.
/// This function takes a proposed schema and validates it against a published
/// schema.
pub async fn run(
    input: CheckWorkflowInput,
    subgraph: String,
    client: &StudioClient,
) -> Result<CheckWorkflowResponse, RoverClientError> {
    let graph_ref = input.graph_ref.clone();
    let mut url: Option<String> = None;
    let now = Instant::now();
    loop {
        let result = client
            .post::<SubgraphCheckWorkflowQuery>(input.clone().into())
            .await;
        match result {
            Ok(data) => {
                let graph = data.clone().graph.ok_or(RoverClientError::GraphNotFound {
                    graph_ref: graph_ref.clone(),
                })?;
                if let Some(check_workflow) = graph.check_workflow {
                    if !matches!(check_workflow.status, CheckWorkflowStatus::PENDING) {
                        return get_check_response_from_data(data, graph_ref, subgraph);
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
    subgraph: String,
) -> Result<CheckWorkflowResponse, RoverClientError> {
    let graph = data.graph.ok_or(RoverClientError::GraphNotFound {
        graph_ref: graph_ref.clone(),
    })?;
    let check_workflow = graph
        .check_workflow
        .ok_or(RoverClientError::GraphNotFound {
            graph_ref: graph_ref.clone(),
        })?;

    let mut core_schema_modified = false;
    let mut composition_errors = Vec::new();

    let mut operations_status = None;
    let mut operations_result: Option<
        SubgraphCheckWorkflowQueryGraphCheckWorkflowTasksOnOperationsCheckTaskResult,
    > = None;
    let mut operations_target_url = None;
    let mut number_of_checked_operations: u64 = 0;

    let mut lint_status = None;
    let mut lint_result: Option<
        SubgraphCheckWorkflowQueryGraphCheckWorkflowTasksOnLintCheckTaskResult,
    > = None;
    let mut lint_target_url = None;

    let mut proposals_status = None;
    let mut proposals_result: Option<ProposalsCheckTaskUnion> = None;
    let mut proposals_target_url = None;

    let mut custom_status = None;
    let mut custom_result: Option<
        SubgraphCheckWorkflowQueryGraphCheckWorkflowTasksOnCustomCheckTaskResult,
    > = None;
    let mut custom_target_url = None;

    let mut downstream_status = None;
    let mut downstream_target_url = None;
    let mut downstream_result: Option<
        Vec<SubgraphCheckWorkflowQueryGraphCheckWorkflowTasksOnDownstreamCheckTaskResults>,
    > = None;

    for task in check_workflow.tasks {
        match task.on {
            CompositionCheckTask(typed_task) => {
                core_schema_modified = typed_task.core_schema_modified;
                if let Some(result) = typed_task.result {
                    composition_errors = result.errors;
                }
                if !composition_errors.is_empty() {
                    break;
                }
            }
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
            ProposalsCheckTask(typed_task) => {
                proposals_status = Some(task.status);
                proposals_target_url = task.target_url;
                proposals_result = Some(typed_task);
            }
            CustomCheckTask(typed_task) => {
                custom_status = Some(task.status);
                custom_target_url = task.target_url;
                custom_result = typed_task.result;
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

    if !composition_errors.is_empty() {
        let num_failures = composition_errors.len();

        let mut build_errors = Vec::with_capacity(num_failures);
        for query_composition_error in composition_errors {
            build_errors.push(BuildError::composition_error(
                query_composition_error.code,
                Some(query_composition_error.message),
                None,
                None,
            ));
        }
        return Err(RoverClientError::SubgraphBuildErrors {
            subgraph,
            graph_ref,
            source: build_errors.into(),
        });
    }

    // Note that graph IDs and variants don't need percent-encoding due to their regex restrictions.
    let default_target_url = format!(
        "https://studio.apollographql.com/graph/{}/variant/{}/checks/variant",
        graph_ref.name, graph_ref.variant
    );

    let check_response = CheckWorkflowResponse {
        default_target_url,
        maybe_core_schema_modified: Some(core_schema_modified),
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
        maybe_proposals_response: get_proposals_response_from_result(
            proposals_target_url,
            proposals_status,
            proposals_result,
        ),
        maybe_custom_response: get_custom_response_from_result(
            custom_status,
            custom_target_url,
            custom_result,
        ),
        maybe_downstream_response: get_downstream_response_from_result(
            downstream_status,
            downstream_target_url,
            downstream_result,
        ),
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
                target_url = task.target_url;
            }
        }
    }
    target_url
}

fn get_operations_response_from_result(
    target_url: Option<String>,
    number_of_checked_operations: u64,
    task_status: CheckWorkflowTaskStatus,
    results: Option<SubgraphCheckWorkflowQueryGraphCheckWorkflowTasksOnOperationsCheckTaskResult>,
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
    results: Option<SubgraphCheckWorkflowQueryGraphCheckWorkflowTasksOnLintCheckTaskResult>,
) -> Option<LintCheckResponse> {
    match results {
        Some(result) => {
            let mut diagnostics = Vec::with_capacity(result.diagnostics.len());
            for diagnostic in result.diagnostics {
                // loc 0 is supergraph and 1 is subgraph
                let mut start_line = 0;
                let mut start_byte_offset = 0;
                let mut end_byte_offset = 0;
                match diagnostic.source_locations.len() {
                    2 => {
                        if let Some(start) = &diagnostic.source_locations[1].start {
                            start_line = start.line;
                            start_byte_offset = start.byte_offset;
                        }
                        if let Some(end) = &diagnostic.source_locations[1].end {
                            end_byte_offset = end.byte_offset;
                        }
                    }
                    _ => {
                        if let Some(start) = &diagnostic.source_locations[0].start {
                            start_line = start.line;
                            start_byte_offset = start.byte_offset;
                        }
                        if let Some(end) = &diagnostic.source_locations[0].end {
                            end_byte_offset = end.byte_offset;
                        }
                    }
                };
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

fn get_proposals_response_from_result(
    target_url: Option<String>,
    task_status: Option<CheckWorkflowTaskStatus>,
    task: Option<ProposalsCheckTaskUnion>,
) -> Option<ProposalsCheckResponse> {
    match task {
        Some(result) => {
            let related_proposals: Vec<RelatedProposal> = result
                .related_proposal_results
                .iter()
                .map(|proposal| {
                    let status = match proposal.status_at_check {
                        ProposalStatus::APPROVED => "APPROVED",
                        ProposalStatus::CLOSED => "CLOSED",
                        ProposalStatus::DRAFT => "DRAFT",
                        ProposalStatus::IMPLEMENTED => "IMPLEMENTED",
                        ProposalStatus::OPEN => "OPEN",
                        _ => "OTHER",
                    };
                    RelatedProposal {
                        status: status.to_string(),
                        display_name: proposal.proposal.display_name.clone(),
                    }
                })
                .collect();
            let severity = match result.severity_level {
                subgraph_check_workflow_query::ProposalChangeMismatchSeverity::ERROR => {
                    ProposalsCheckSeverityLevel::ERROR
                }
                subgraph_check_workflow_query::ProposalChangeMismatchSeverity::OFF => {
                    ProposalsCheckSeverityLevel::OFF
                }
                subgraph_check_workflow_query::ProposalChangeMismatchSeverity::WARN => {
                    ProposalsCheckSeverityLevel::WARN
                }
                _ => ProposalsCheckSeverityLevel::OFF,
            };
            let coverage = match result.proposal_coverage {
                subgraph_check_workflow_query::ProposalCoverage::FULL => ProposalsCoverage::FULL,
                subgraph_check_workflow_query::ProposalCoverage::PARTIAL => {
                    ProposalsCoverage::PARTIAL
                }
                subgraph_check_workflow_query::ProposalCoverage::NONE => ProposalsCoverage::NONE,
                subgraph_check_workflow_query::ProposalCoverage::OVERRIDDEN => {
                    ProposalsCoverage::OVERRIDDEN
                }
                subgraph_check_workflow_query::ProposalCoverage::PENDING => {
                    ProposalsCoverage::PENDING
                }
                _ => ProposalsCoverage::PENDING,
            };
            Some(ProposalsCheckResponse {
                target_url,
                task_status: task_status.into(),
                severity_level: severity,
                proposal_coverage: coverage,
                related_proposals,
            })
        }
        None => None,
    }
}

fn get_custom_response_from_result(
    task_status: Option<CheckWorkflowTaskStatus>,
    target_url: Option<String>,
    results: Option<SubgraphCheckWorkflowQueryGraphCheckWorkflowTasksOnCustomCheckTaskResult>,
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
        Vec<SubgraphCheckWorkflowQueryGraphCheckWorkflowTasksOnDownstreamCheckTaskResults>,
    >,
) -> Option<DownstreamCheckResponse> {
    match results {
        Some(results) => {
            let blocking_variants = results
                .iter()
                .filter(|result| result.fails_upstream_workflow.unwrap_or(false))
                .map(|result| result.downstream_variant_name.clone())
                .collect();
            Some(DownstreamCheckResponse {
                task_status: task_status.into(),
                target_url,
                blocking_variants,
            })
        }
        None => None,
    }
}

#[cfg(test)]
#[expect(clippy::panic)]
mod tests {
    use super::*;
    use serde_json::json;

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
    fn test_get_check_response_from_data_with_passed_status() {
        let data = create_check_workflow_data(CheckWorkflowStatus::PASSED, json!([]));
        let graph_ref = "test-graph@test-variant".parse().unwrap();
        let subgraph = "test-subgraph".to_string();

        let result = get_check_response_from_data(data, graph_ref, subgraph);

        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(
            response.default_target_url,
            "https://studio.apollographql.com/graph/test-graph/variant/test-variant/checks/variant"
        );
        assert_eq!(response.maybe_core_schema_modified, Some(false));
        assert!(response.maybe_operations_response.is_none());
        assert!(response.maybe_lint_response.is_none());
        assert!(response.maybe_proposals_response.is_none());
        assert!(response.maybe_custom_response.is_none());
        assert!(response.maybe_downstream_response.is_none());
    }

    #[test]
    fn test_get_check_response_from_data_with_failed_status() {
        let data = create_check_workflow_data(CheckWorkflowStatus::FAILED, json!([]));
        let graph_ref: GraphRef = "test-graph@test-variant".parse().unwrap();
        let subgraph = "test-subgraph".to_string();

        let result = get_check_response_from_data(data, graph_ref.clone(), subgraph);

        assert!(result.is_err());
        match result.unwrap_err() {
            RoverClientError::CheckWorkflowFailure {
                graph_ref: returned_graph_ref,
                check_response,
            } => {
                assert_eq!(returned_graph_ref, graph_ref);
                assert_eq!(
                    check_response.default_target_url,
                    "https://studio.apollographql.com/graph/test-graph/variant/test-variant/checks/variant"
                );
            }
            _ => panic!("Expected CheckWorkflowFailure error"),
        }
    }

    #[test]
    fn test_get_check_response_from_data_with_composition_errors() {
        let data = create_check_workflow_data(
            CheckWorkflowStatus::FAILED,
            json!([
                {
                    "__typename": "CompositionCheckTask",
                    "id": "composition-task",
                    "status": "FAILED",
                    "targetUrl": null,
                    "coreSchemaModified": false,
                    "result": {
                        "__typename": "CompositionCheckResult",
                        "errors": [
                            {
                                "code": "INVALID_GRAPHQL",
                                "message": "Type 'User' is missing field 'id'",
                                "locations": []
                            }
                        ]
                    }
                }
            ]),
        );
        let graph_ref: GraphRef = "test-graph@test-variant".parse().unwrap();
        let subgraph = "test-subgraph".to_string();

        let result = get_check_response_from_data(data, graph_ref.clone(), subgraph.clone());

        assert!(result.is_err());
        match result.unwrap_err() {
            RoverClientError::SubgraphBuildErrors {
                subgraph: returned_subgraph,
                graph_ref: returned_graph_ref,
                source,
            } => {
                assert_eq!(returned_subgraph, subgraph);
                assert_eq!(returned_graph_ref, graph_ref);
                assert!(source.to_string().contains("INVALID_GRAPHQL"));
            }
            _ => panic!("Expected SubgraphBuildErrors error"),
        }
    }

    #[test]
    fn test_get_check_response_from_data_with_null_operations_result() {
        let data = create_check_workflow_data(
            CheckWorkflowStatus::PASSED,
            json!([
                {
                    "__typename": "OperationsCheckTask",
                    "id": "operations-task",
                    "status": "PENDING",
                    "targetUrl": "https://studio.apollographql.com/graph/test/checks/operations",
                    "result": null // This will be null when the task is initializing or running, or when the composition check task fails (https://studio.apollographql.com/graph/apollo-platform/variant/main/schema/reference/objects/OperationsCheckTask#result)
                }
            ]),
        );
        let graph_ref: GraphRef = "test-graph@test-variant".parse().unwrap();
        let subgraph = "test-subgraph".to_string();

        let result = get_check_response_from_data(data, graph_ref, subgraph);

        // Should succeed instead of returning MalformedResponse error
        assert!(result.is_ok());
        let response = result.unwrap();

        // Operations response should be None since result was null
        assert!(response.maybe_operations_response.is_none());
    }

    #[test]
    fn test_get_check_response_from_data_with_null_lint_result() {
        let data = create_check_workflow_data(
            CheckWorkflowStatus::PASSED,
            json!([
                {
                    "__typename": "LintCheckTask",
                    "id": "lint-task",
                    "status": "PENDING",
                    "targetUrl": "https://studio.apollographql.com/graph/test/checks/lint",
                    "result": null // This is also nullable (https://studio.apollographql.com/graph/apollo-platform/variant/main/schema/reference/objects/LintCheckTask#result)
                }
            ]),
        );

        let graph_ref: GraphRef = "test-graph@test-variant".parse().unwrap();
        let subgraph = "test-subgraph".to_string();

        let result = get_check_response_from_data(data, graph_ref, subgraph);

        // Should succeed instead of returning MalformedResponse error
        assert!(result.is_ok());
        let response = result.unwrap();

        // Lint response should be None since result was null
        assert!(response.maybe_lint_response.is_none());
    }
}

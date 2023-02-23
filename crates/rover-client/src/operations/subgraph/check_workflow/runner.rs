use std::time::{Duration, Instant};

use super::types::*;
use crate::blocking::StudioClient;
use crate::operations::subgraph::check_workflow::types::QueryResponseData;
use crate::shared::{
    CheckResponse, GraphRef, OperationCheckResponse, SchemaChange, SkipOperationsCheckResponse,
};
use crate::RoverClientError;

use apollo_federation_types::build::BuildError;

use graphql_client::*;

use self::subgraph_check_workflow_query::CheckWorkflowStatus;
use self::subgraph_check_workflow_query::CheckWorkflowTaskStatus;
use self::subgraph_check_workflow_query::SubgraphCheckWorkflowQueryGraphCheckWorkflowTasksOn::{
    CompositionCheckTask, DownstreamCheckTask, OperationsCheckTask,
};
use self::subgraph_check_workflow_query::SubgraphCheckWorkflowQueryGraphCheckWorkflowTasksOnOperationsCheckTaskResult;

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
pub fn run(
    input: CheckWorkflowInput,
    subgraph: String,
    client: &StudioClient,
) -> Result<CheckResponse, RoverClientError> {
    let graph_ref = input.graph_ref.clone();
    let mut data;
    let now = Instant::now();
    loop {
        data = client.post::<SubgraphCheckWorkflowQuery>(input.clone().into())?;
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
    get_check_response_from_data(data, graph_ref, subgraph)
}

fn get_check_response_from_data(
    data: QueryResponseData,
    graph_ref: GraphRef,
    subgraph: String,
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
    let mut operations_result = None;
    let mut number_of_checked_operations: u64 = 0;
    let mut core_schema_modified = false;
    let mut composition_errors = Vec::new();
    let mut downstream_status = None;
    let mut downstream_target_url = None;
    let mut blocking_downstream_variants = Vec::new();

    // This will either be the operation check target url if present or the composition check target url if present or None if none are found
    let mut display_target_url = None;
    for task in check_workflow.tasks {
        match task.on {
            CompositionCheckTask(typed_task) => {
                core_schema_modified = typed_task.core_schema_modified;
                if let Some(result) = typed_task.result {
                    composition_errors = result.errors;
                }
                if display_target_url.is_none() {
                    display_target_url = task.target_url;
                }
                if !composition_errors.is_empty() {
                    break;
                }
            }
            OperationsCheckTask(typed_task) => {
                operations_status = Some(task.status);
                display_target_url = task.target_url;
                if let Some(result) = typed_task.result {
                    number_of_checked_operations =
                        result.number_of_checked_operations.try_into().unwrap();
                    operations_result = Some(result);
                } else {
                    return Err(RoverClientError::MalformedResponse {
                        null_field: "graph.checkWorkflow....on OperationsCheckTask.result"
                            .to_string(),
                    });
                }
            }
            DownstreamCheckTask(typed_task) => {
                downstream_status = Some(task.status);
                downstream_target_url = task.target_url;
                if let Some(results) = typed_task.results {
                    blocking_downstream_variants = results
                        .iter()
                        .filter(|result| result.fails_upstream_workflow.unwrap_or(false))
                        .map(|result| result.downstream_variant_name.clone())
                        .collect();
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
        "https://studio.apollographql.com/graph/{}/checks?variant={}",
        graph_ref.name, graph_ref.variant
    );

    if matches!(operations_status, Some(CheckWorkflowTaskStatus::FAILED)) {
        get_check_response_from_result(
            operations_result,
            display_target_url,
            number_of_checked_operations,
            workflow_status,
            graph_ref,
            core_schema_modified,
        )
    } else if matches!(downstream_status, Some(CheckWorkflowTaskStatus::FAILED)) {
        Err(RoverClientError::DownstreamCheckFailure {
            blocking_downstream_variants,
            target_url: downstream_target_url.unwrap_or(default_target_url),
        })
    } else if matches!(workflow_status, CheckWorkflowStatus::PASSED) {
        get_check_response_from_result(
            operations_result,
            display_target_url,
            number_of_checked_operations,
            workflow_status,
            graph_ref,
            core_schema_modified,
        )
    } else {
        Err(RoverClientError::OtherCheckTaskFailure {
            has_build_task: true,
            has_downstream_task: downstream_status.is_some(),
            target_url: display_target_url.unwrap_or(default_target_url),
        })
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

fn get_check_response_from_result(
    maybe_operations_result: Option<
        SubgraphCheckWorkflowQueryGraphCheckWorkflowTasksOnOperationsCheckTaskResult,
    >,
    display_target_url: Option<String>,
    number_of_checked_operations: u64,
    workflow_status: CheckWorkflowStatus,
    graph_ref: GraphRef,
    core_schema_modified: bool,
) -> Result<CheckResponse, RoverClientError> {
    match maybe_operations_result {
        Some(result) => {
            let mut changes = Vec::with_capacity(result.changes.len());
            for change in result.changes {
                changes.push(SchemaChange {
                    code: change.code,
                    severity: change.severity.into(),
                    description: change.description,
                });
            }

            OperationCheckResponse::try_new(
                display_target_url,
                number_of_checked_operations,
                changes,
                workflow_status.into(),
                graph_ref,
                core_schema_modified,
            )
            .map(CheckResponse::OperationCheckResponse)
        }
        None => Ok(CheckResponse::SkipOperationsCheckResponse(
            SkipOperationsCheckResponse {
                target_url: display_target_url,
                core_schema_modified,
            },
        )),
    }
}

use std::time::{Duration, Instant};

use super::types::*;
use crate::blocking::StudioClient;
use crate::operations::subgraph::check_workflow::types::QueryResponseData;
use crate::shared::{CheckResponse, GraphRef, SchemaChange};
use crate::RoverClientError;

use apollo_federation_types::build::BuildError;

use graphql_client::*;

use self::subgraph_check_workflow_query::CheckWorkflowStatus;
use self::subgraph_check_workflow_query::SubgraphCheckWorkflowQueryGraphCheckWorkflowTasksOn::{
    CompositionCheckTask, OperationsCheckTask,
};

#[derive(GraphQLQuery)]
// The paths are relative to the directory where your `Cargo.toml` is located.
// Both json and the GraphQL schema language are supported as sources for the schema
#[graphql(
    query_path = "src/operations/subgraph/check_workflow/check_workflow_query.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "PartialEq, Debug, Serialize, Deserialize, Clone",
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
            return Err(RoverClientError::ChecksTimeoutError);
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

    let status = check_workflow.status.into();
    let mut operations_result = None;
    let mut target_url = None;
    let mut number_of_checked_operations: u64 = 0;
    let mut core_schema_modified = false;
    let mut composition_errors = Vec::new();
    for task in check_workflow.tasks {
        match task.on {
            OperationsCheckTask(typed_task) => {
                target_url = task.target_url;
                if let Some(result) = typed_task.result {
                    number_of_checked_operations =
                        result.number_of_checked_operations.try_into().unwrap();
                    operations_result = Some(result);
                }
            }
            CompositionCheckTask(typed_task) => {
                core_schema_modified = typed_task.core_schema_modified;
                if let Some(result) = typed_task.result {
                    composition_errors = result.errors;
                }
            }
            _ => (),
        }
    }

    if composition_errors.is_empty() {
        let result = operations_result.ok_or(RoverClientError::AdhocError {
            msg: "No operation was found for this check.".to_string(),
        })?;

        let mut changes = Vec::with_capacity(result.changes.len());
        for change in result.changes {
            changes.push(SchemaChange {
                code: change.code,
                severity: change.severity.into(),
                description: change.description,
            });
        }

        CheckResponse::try_new(
            target_url,
            number_of_checked_operations,
            changes,
            status,
            graph_ref,
            core_schema_modified,
        )
    } else {
        let num_failures = composition_errors.len();

        let mut build_errors = Vec::with_capacity(num_failures);
        for query_composition_error in composition_errors {
            build_errors.push(BuildError::composition_error(
                query_composition_error.code,
                Some(query_composition_error.message),
            ));
        }
        Err(RoverClientError::SubgraphBuildErrors {
            subgraph,
            graph_ref,
            source: build_errors.into(),
        })
    }
}

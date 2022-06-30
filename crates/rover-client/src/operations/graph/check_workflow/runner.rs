use std::time::{Instant, Duration};

use crate::blocking::StudioClient;
use crate::operations::graph::check_workflow::types::{CheckWorkflowInput, QueryResponseData};
use crate::shared::{CheckResponse, GraphRef, SchemaChange, ChangeSeverity};
use crate::RoverClientError;

use graphql_client::*;

use self::graph_check_workflow_query::GraphCheckWorkflowQueryGraphCheckWorkflowTasks::OperationsCheckTask;
use self::graph_check_workflow_query::{GraphCheckWorkflowQueryGraphCheckWorkflowTasksOnOperationsCheckTaskResult, CheckWorkflowStatus};

#[derive(GraphQLQuery)]
// The paths are relative to the directory where your `Cargo.toml` is located.
// Both json and the GraphQL schema language are supported as sources for the schema
#[graphql(
    query_path = "src/operations/graph/check_workflow/check_workflow_query.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "PartialEq, Debug, Serialize, Deserialize, Clone",
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
    // default timeout is 5 minutes
    let timeout_seconds = option_env!("APOLLO_CHECKS_TIMEOUT_SECONDS")
    .unwrap_or_else(|| "300").parse::<u64>().unwrap();
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
        if now.elapsed() > Duration::from_secs(timeout_seconds) {
            // TODO timeout error
            eprintln!("Timeout after {} seconds waiting for check to complete, check again later.", timeout_seconds);
            break;
        }
        std::thread::sleep(Duration::from_secs(5));
    };
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
        .ok_or(RoverClientError::GraphNotFound { graph_ref: graph_ref.clone() })?;

    let mut operations_result:Option<GraphCheckWorkflowQueryGraphCheckWorkflowTasksOnOperationsCheckTaskResult> = None;
    let mut target_url = None;
    let mut status = ChangeSeverity::FAIL;
    let mut number_of_checked_operations:u64 = 0;
    for task in check_workflow.tasks {
        if let OperationsCheckTask(task) = task {
            target_url = task.target_url;
            status = task.status.into();
            if let Some(result) = task.result {
                number_of_checked_operations = result.number_of_checked_operations.try_into().unwrap();
                operations_result = Some(result);
            }
        }
    }

    let result = operations_result.ok_or(RoverClientError::AdhocError{
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

    // The `graph` check response does not return this field
    // only `subgraph` check does. Since `CheckResponse` is shared
    // between `graph` and `subgraph` checks, defaulting this
    // to false for now since its currently only used in
    // `check_response.rs` to format better console messages.
    let core_schema_modified = false;

    CheckResponse::try_new(
        target_url,
        number_of_checked_operations,
        changes,
        status,
        graph_ref,
        core_schema_modified,
    )
}

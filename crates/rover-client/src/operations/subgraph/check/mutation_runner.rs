use super::types::*;
use crate::blocking::StudioClient;
use crate::operations::{config::is_federated, subgraph::check::types::MutationResponseData};
use crate::shared::{CheckResponse, SchemaChange};
use crate::RoverClientError;

use graphql_client::*;

type Timestamp = String;
#[derive(GraphQLQuery)]
// The paths are relative to the directory where your `Cargo.toml` is located.
// Both json and the GraphQL schema language are supported as sources for the schema
#[graphql(
    query_path = "src/operations/subgraph/check/check_mutation.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
/// This struct is used to generate the module containing `Variables` and
/// `ResponseData` structs.
/// Snake case of this name is the mod name. i.e. subgraph_check_mutation
pub(crate) struct SubgraphCheckMutation;

/// The main function to be used from this module.
/// This function takes a proposed schema and validates it against a published
/// schema.
pub fn run(
    input: SubgraphCheckInput,
    client: &StudioClient,
) -> Result<CheckResponse, RoverClientError> {
    let graph = input.graph_id.clone();
    // This response is used to check whether or not the current graph is federated.
    let is_federated = is_federated::run(
        is_federated::is_federated_graph::Variables {
            graph_id: input.graph_id.clone(),
            graph_variant: input.variant.clone(),
        },
        &client,
    )?;
    if !is_federated {
        return Err(RoverClientError::ExpectedFederatedGraph {
            graph,
            can_operation_convert: false,
        });
    }
    let variables = input.into();
    let data = client.post::<SubgraphCheckMutation>(variables)?;
    get_check_response_from_data(data, graph)
}

fn get_check_response_from_data(
    data: MutationResponseData,
    graph_name: String,
) -> Result<CheckResponse, RoverClientError> {
    let service = data.service.ok_or(RoverClientError::NoService {
        graph: graph_name.clone(),
    })?;

    // for some reason this is a `Vec<Option<CompositionError>>`
    // we convert this to just `Vec<CompositionError>` because the `None`
    // errors would be useless.
    let query_composition_errors: Vec<MutationCompositionErrors> = service
        .check_partial_schema
        .composition_validation_result
        .errors;

    if query_composition_errors.is_empty() {
        let check_schema_result = service.check_partial_schema.check_schema_result.ok_or(
            RoverClientError::MalformedResponse {
                null_field: "service.check_partial_schema.check_schema_result".to_string(),
            },
        )?;

        let diff_to_previous = check_schema_result.diff_to_previous;

        let number_of_checked_operations =
            diff_to_previous.number_of_checked_operations.unwrap_or(0);

        let change_severity = diff_to_previous.severity.into();

        let mut changes = Vec::with_capacity(diff_to_previous.changes.len());
        let mut num_failures = 0;
        for change in diff_to_previous.changes {
            if let MutationChangeSeverity::FAILURE = change.severity {
                num_failures += 1;
            }
            changes.push(SchemaChange {
                code: change.code,
                severity: change.severity.into(),
                description: change.description,
            });
        }

        let check_response = CheckResponse {
            num_failures,
            target_url: check_schema_result.target_url,
            number_of_checked_operations,
            changes,
            change_severity,
        };
        check_response.check_for_failures()
    } else {
        let num_failures = query_composition_errors.len();

        let mut composition_errors = Vec::with_capacity(num_failures);
        for query_composition_error in query_composition_errors {
            composition_errors.push(CompositionError {
                message: query_composition_error.message,
                code: query_composition_error.code,
            });
        }
        Err(RoverClientError::SubgraphCompositionErrors {
            graph_name,
            composition_errors,
        })
    }
}

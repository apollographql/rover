use std::fmt;

use crate::blocking::StudioClient;
use crate::operations::config::is_federated::{self, IsFederatedInput};
use crate::operations::subgraph::fetch;
use crate::operations::subgraph::fetch::SubgraphFetchInput;
use crate::operations::subgraph::lint::types::{
    LintResponseData, LintSubgraphInput, LintSubgraphMutationInput,
};
use crate::shared::{Diagnostic, GraphRef, LintResponse};
use crate::RoverClientError;

use graphql_client::*;

#[derive(GraphQLQuery)]
// The paths are relative to the directory where your `Cargo.toml` is located.
// Both json and the GraphQL schema language are supported as sources for the schema
#[graphql(
    query_path = "src/operations/subgraph/lint/lint_subgraph_mutation.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "PartialEq, Eq, Debug, Serialize, Deserialize, Clone",
    deprecated = "warn"
)]
/// This struct is used to generate the module containing `Variables` and
/// `ResponseData` structs.
/// Snake case of this name is the mod name. i.e. lint_subgraph_mutation
pub(crate) struct LintSubgraphMutation;

/// The main function to be used from this module.
/// This function takes a proposed schema and validates it against a published
/// schema.
pub fn run(
    input: LintSubgraphInput,
    client: &StudioClient,
) -> Result<LintResponse, RoverClientError> {
    let graph_ref = input.graph_ref.clone();
    // This response is used to check whether or not the current graph is federated.
    let is_federated = is_federated::run(
        IsFederatedInput {
            graph_ref: graph_ref.clone(),
        },
        client,
    )?;
    if !is_federated {
        return Err(RoverClientError::ExpectedFederatedGraph {
            graph_ref,
            can_operation_convert: false,
        });
    }

    let base_schema = if input.ignore_existing {
        let fetch_response = fetch::run(
            SubgraphFetchInput {
                graph_ref: graph_ref.clone(),
                subgraph_name: input.subgraph_name,
            },
            client,
        )?;
        Some(fetch_response.sdl.contents)
    } else {
        None
    };

    let data = client.post::<LintSubgraphMutation>(
        LintSubgraphMutationInput {
            graph_ref,
            proposed_schema: input.proposed_schema,
            base_schema,
        }
        .into(),
    )?;

    get_lint_response_from_result(data, input.graph_ref)
}

fn get_lint_response_from_result(
    result: LintResponseData,
    graph_ref: GraphRef,
) -> Result<LintResponse, RoverClientError> {
    if let Some(maybe_graph) = result.graph {
        let mut diagnostics: Vec<Diagnostic> = Vec::new();
        for diagnostic in maybe_graph.lint_schema.diagnostics {
            let mut start_line = 0;
            let mut column = 0;
            if let Some(start) = &diagnostic.source_locations[0].start {
                start_line = start.line;
                column = start.column;
            }
            diagnostics.push(Diagnostic {
                level: diagnostic.level.to_string(),
                message: diagnostic.message,
                coordinate: diagnostic.coordinate,
                line: start_line.unsigned_abs(),
                column: column.unsigned_abs(),
            })
        }
        if maybe_graph.lint_schema.stats.errors_count > 0 {
            let lint_response = LintResponse { diagnostics };
            Err(RoverClientError::LintFailures { lint_response })
        } else {
            Ok(LintResponse { diagnostics })
        }
    } else {
        Err(RoverClientError::GraphNotFound { graph_ref })
    }
}

impl fmt::Display for lint_subgraph_mutation::LintDiagnosticLevel {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let printable = match &self {
            lint_subgraph_mutation::LintDiagnosticLevel::WARNING => "WARNING",
            lint_subgraph_mutation::LintDiagnosticLevel::ERROR => "ERROR",
            lint_subgraph_mutation::LintDiagnosticLevel::IGNORED => "IGNORED",
            lint_subgraph_mutation::LintDiagnosticLevel::Other(_) => "UNKNOWN",
        };
        write!(f, "{}", printable)
    }
}

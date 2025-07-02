use std::fmt;

use graphql_client::*;

use crate::blocking::StudioClient;
use crate::operations::graph::fetch;
use crate::operations::graph::fetch::GraphFetchInput;
use crate::operations::graph::lint::types::{
    LintGraphInput, LintGraphMutationInput, LintResponseData,
};
use crate::shared::{Diagnostic, GraphRef, LintResponse};
use crate::RoverClientError;

#[derive(GraphQLQuery)]
// The paths are relative to the directory where your `Cargo.toml` is located.
// Both json and the GraphQL schema language are supported as sources for the schema
#[graphql(
    query_path = "src/operations/graph/lint/lint_graph_mutation.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "PartialEq, Eq, Debug, Serialize, Deserialize, Clone",
    deprecated = "warn"
)]
/// This struct is used to generate the module containing `Variables` and
/// `ResponseData` structs.
/// Snake case of this name is the mod name. i.e. lint_graph_mutation
pub(crate) struct LintGraphMutation;

/// The main function to be used from this module.
/// This function takes a proposed schema and validates it against a published
/// schema.
pub async fn run(
    input: LintGraphInput,
    client: &StudioClient,
) -> Result<LintResponse, RoverClientError> {
    let graph_ref = input.graph_ref.clone();

    let base_schema = if input.ignore_existing {
        let fetch_response = fetch::run(
            GraphFetchInput {
                graph_ref: graph_ref.clone(),
            },
            client,
        )
        .await?;
        Some(fetch_response.sdl.contents)
    } else {
        None
    };

    let data = client
        .post::<LintGraphMutation>(
            LintGraphMutationInput {
                graph_ref,
                proposed_schema: input.proposed_schema.clone(),
                base_schema,
            }
            .into(),
        )
        .await?;

    get_lint_response_from_result(
        data,
        input.graph_ref,
        input.file_name,
        input.proposed_schema,
    )
}

fn get_lint_response_from_result(
    result: LintResponseData,
    graph_ref: GraphRef,
    file_name: String,
    proposed_schema: String,
) -> Result<LintResponse, RoverClientError> {
    if let Some(maybe_graph) = result.graph {
        let mut diagnostics: Vec<Diagnostic> = Vec::new();
        for diagnostic in maybe_graph.lint_schema.diagnostics {
            let mut start_line = 0;
            let mut start_byte_offset = 0;
            let mut end_byte_offset = 0;
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
        if maybe_graph.lint_schema.stats.errors_count > 0 {
            let lint_response = LintResponse {
                diagnostics,
                file_name,
                proposed_schema,
            };
            Err(RoverClientError::LintFailures { lint_response })
        } else {
            Ok(LintResponse {
                diagnostics,
                file_name,
                proposed_schema,
            })
        }
    } else {
        Err(RoverClientError::GraphNotFound { graph_ref })
    }
}

impl fmt::Display for lint_graph_mutation::LintDiagnosticLevel {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let printable = match &self {
            lint_graph_mutation::LintDiagnosticLevel::WARNING => "WARNING",
            lint_graph_mutation::LintDiagnosticLevel::ERROR => "ERROR",
            lint_graph_mutation::LintDiagnosticLevel::IGNORED => "IGNORED",
            lint_graph_mutation::LintDiagnosticLevel::Other(_) => "UNKNOWN",
        };
        write!(f, "{printable}")
    }
}

use anyhow::anyhow;
use clap::Parser;
use rover_std::Style;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use std::fmt::Write;
use std::path::PathBuf;

use crate::composition::supergraph::binary::SupergraphBinary;
use crate::utils::effect::exec::TokioCommand;
use crate::utils::table;
use crate::{RoverOutput, RoverResult};

#[derive(Debug, Parser, Clone, Serialize)]
pub struct RunConnector {
    /// The path to the schema file containing the connector.
    ///
    /// Optional if there is a `supergraph.yaml` containing only a single subgraph
    #[arg(long = "schema")]
    schema_path: Option<PathBuf>,
    /// The ID of the connector to run, which can be:
    ///
    /// 1. The name of a type, like `MyType`, if the connector is on the type
    /// 2. A type & field, like `Query.myField`, if the connector is on a field
    /// 3. One of the above with an [index], like `Query.myField[1]`, if there are multiple connectors on the same element
    ///
    /// Use `rover connector list` to see available connectors and their IDs.
    #[arg(short = 'c', long = "connector-id")]
    connector_id: String,
    /// JSON data containing the variables required by the connector.
    /// For example: `'{"$args": {"id": "123"}}'`
    #[arg(short = 'v', long = "variables")]
    variables: String,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug, Eq)]
pub struct RunConnectorOutput {
    request: Option<Request>,
    response: Option<Response>,
    error: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug, Eq)]
struct Request {
    // TODO: Make a dedicated struct rather than Rover relying on apollo-federation
    method: String,
    uri: String,
    headers: Value,
    problems: Vec<Problem>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug, Eq)]
struct Response {
    body: Value,
    status: u16,
    headers: Value,
    problems: Vec<Problem>,
    mapped_data: Value,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug, Eq)]
struct Problem {
    message: String,
    path: String,
    count: i32,
    location: String,
}

impl RunConnector {
    pub async fn run(
        &self,
        supergraph_binary: SupergraphBinary,
        default_subgraph: Option<PathBuf>,
    ) -> RoverResult<RoverOutput> {
        let exec_command_impl = TokioCommand::default();
        let schema_path = self.schema_path.clone().or(default_subgraph).ok_or_else(|| anyhow!(
            "A schema path must be provided either via --schema or a `supergraph.yaml` containing a single subgraph"
        ))?;
        let result = supergraph_binary
            .run_connector(
                &exec_command_impl,
                schema_path,
                self.connector_id.clone(),
                self.variables.clone(),
            )
            .await?;
        Ok(result)
    }

    pub fn format_output(output: &RunConnectorOutput) -> String {
        if let Some(error) = &output.error {
            let error_prefix = Style::ErrorPrefix.paint("ERROR:");
            return format!("{} {error}", error_prefix);
        }

        let mut result = String::new();

        if let Some(request) = &output.request {
            let request_header = Style::SuccessHeading.paint("Request");

            let mut request_table = table::get_table();
            request_table.add_row(vec![Style::Heading.paint("Method"), request.method.clone()]);
            request_table.add_row(vec![Style::Heading.paint("Uri"), request.uri.clone()]);

            write!(&mut result, "{request_header}\n{request_table}").unwrap();
        }

        if let Some(request) = &output.request
            && let Some(header_object) = request.headers.as_object()
            && !header_object.is_empty()
        {
            let request_headers_header = Style::SuccessHeading.paint("Request Headers");

            let mut request_headers_table = table::get_table();
            request_headers_table.set_header(vec![
                Style::Heading.paint("Header"),
                Style::Heading.paint("Value"),
            ]);

            for (header_name, header_value) in header_object {
                request_headers_table.add_row(vec![
                    header_name,
                    &header_value.as_str().unwrap_or_default().to_string(),
                ]);
            }

            write!(
                &mut result,
                "\n\n{request_headers_header}\n{request_headers_table}"
            )
            .unwrap();
        }

        if let Some(request) = &output.request
            && !request.problems.is_empty()
        {
            let request_problems_header = Style::WarningHeading.paint("Request Mapping Problems");

            let mut request_problems_table = table::get_table();
            request_problems_table.set_header(vec![
                Style::Heading.paint("Message"),
                Style::Heading.paint("Path"),
                Style::Heading.paint("Count"),
                Style::Heading.paint("Location"),
            ]);

            for problem in &request.problems {
                request_problems_table.add_row(vec![
                    problem.message.clone(),
                    problem.path.clone(),
                    problem.count.to_string(),
                    problem.location.clone(),
                ]);
            }

            write!(
                &mut result,
                "\n\n{request_problems_header}\n{request_problems_table}"
            )
            .unwrap();
        }

        if let Some(response) = &output.response {
            let response_header = Style::SuccessHeading.paint("Response");

            let mut response_table = table::get_table();
            response_table.set_header(vec![
                Style::Heading.paint("Status"),
                Style::Heading.paint("Body"),
                Style::Heading.paint("Mapped Data"),
            ]);

            response_table.add_row(vec![
                response.status.to_string(),
                serde_json::to_string_pretty(&response.body).unwrap_or_default(),
                serde_json::to_string_pretty(&response.mapped_data).unwrap_or_default(),
            ]);

            write!(&mut result, "\n\n{response_header}\n{response_table}").unwrap();
        }

        if let Some(response) = &output.response
            && let Some(header_object) = response.headers.as_object()
            && !header_object.is_empty()
        {
            let response_headers_header = Style::SuccessHeading.paint("Response Headers");

            let mut response_headers_table = table::get_table();
            response_headers_table.set_header(vec![
                Style::Heading.paint("Header"),
                Style::Heading.paint("Value"),
            ]);

            for (header_name, header_value) in header_object {
                response_headers_table.add_row(vec![
                    header_name,
                    &header_value.as_str().unwrap_or_default().to_string(),
                ]);
            }

            write!(
                &mut result,
                "\n\n{response_headers_header}\n{response_headers_table}"
            )
            .unwrap();
        }

        if let Some(response) = &output.response
            && !response.problems.is_empty()
        {
            let response_problems_header = Style::WarningHeading.paint("Response Mapping Problems");

            let mut response_problems_table = table::get_table();
            response_problems_table.set_header(vec![
                Style::Heading.paint("Message"),
                Style::Heading.paint("Path"),
                Style::Heading.paint("Count"),
                Style::Heading.paint("Location"),
            ]);

            for problem in &response.problems {
                response_problems_table.add_row(vec![
                    problem.message.clone(),
                    problem.path.clone(),
                    problem.count.to_string(),
                    problem.location.clone(),
                ]);
            }

            write!(
                &mut result,
                "\n\n{response_problems_header}\n{response_problems_table}"
            )
            .unwrap();
        }

        result
    }
}
